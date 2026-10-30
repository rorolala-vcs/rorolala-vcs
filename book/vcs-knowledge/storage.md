---
title: "About Storage System"
description: "An introduction to Rorolala's storage system"
keywords: system, storage, store, chunking, chunk, manifest
---

# About Storage System

The storage system stores *content only*: every object is named by the BLAKE3 hash of its content and retrieved by that hash — this is **CAS (content-addressable storage)**.

Rorolala has two storages that share the same underlying layer:

- **Content storage:** it holds the content of project files.
- **Version control index:** it holds variants, versions, Creators and Messages; see `vcs-knowledge/variant` and `vcs-knowledge/version`.

Both are addressed by key and both use the same frames, keys and packing directory layout — they simply store *different things*.

## How addressing works

Every object in storage is named by a key, and the key is the BLAKE3 hash of the object's **original content**.

**Note:** it is *not* the hash of the compressed bytes, nor the hash of the file on disk. From this we get three properties:

- **Same content, same key:** a second write of identical content takes no extra space.
- **Content is immutable:** change the content and the hash becomes a different key; the old key still points at the old content.
- **Reads verify the bytes against the key:** if they don't match, the data is corrupt and will never be handed out.

A key is 32 bytes, and the command line accepts three forms:

| Form                       | Meaning                                                                  |
| -------------------------- | ------------------------------------------------------------------------ |
| `blake3:<64 hex digits>`   | A content object                                                         |
| `manifest:<64 hex digits>` | A chunked piece of content; the digest is still the whole content's hash |
| `<64 hex digits>`          | A prefix-free form, equivalent to `blake3:`                              |

`manifest:` is *not* another kind of key — it is only a different annotation of the same digest: it tells the reader that this content is stored as a manifest plus several chunks. When content is fetched, either form reads it back.

Commands also accept the beginning of a hash, at least 4 hex digits. A short name is resolved only among the objects the current command can enumerate; if it matches nothing or matches more than one, the command errors out rather than picking one for you.

## Write flow

```text
content
  ├─ choose codec: Raw / Lz4 / Zstd
  ├─ choose chunking: Whole / Fixed / Cdc / Text / Zip / Policy
  └─ write
       ├─ one chunk: one object
       └─ many chunks: one object per chunk, plus a manifest
```

Codec and chunking are *two independent decisions*. The codec decides how the payload is compressed; the chunking decides how the content is split into chunks. **Neither is written into the key**, so the same content stored a different way is still the same key.

Chunking happens *only at write time*. A read needs only the manifest, and never needs to know which chunking was used at the time — see `vcs-knowledge/chunk` and `vcs-knowledge/manifest`.

## Storage directory

A directory is a storage directory as soon as it contains a `rolast.toml`.

Content storage usually appears in two places:

| Location                     | Purpose                 |
| ---------------------------- | ----------------------- |
| `<Vault>/storage/`           | The vault's own storage |
| `<Workspace>/.rola/storage/` | The workspace's storage |

The layout inside a storage is *its own business*; callers only hand over content and a key:

| Path                       | What it holds                           |
| -------------------------- | --------------------------------------- |
| `rolast.toml`              | The config file that makes it a storage |
| `obj/<hh>/<hh>/<hex>`      | Loose objects                           |
| `manifest/<hh>/<hh>/<hex>` | The manifests of chunked content        |
| `packed/packed_<n>.pack`   | A pack, containing many entries         |
| `packed/packed_<n>.idx`    | The index of an object pack             |
| `manifest/packed_<n>.idx`  | The index of a manifest pack            |

Paths under `obj/` and `manifest/` are sharded by the first two levels of the digest's hex digits, two characters each, so *no single directory accumulates an unbounded number of entries*. A pack holds only one kind of entry: object packs or manifest packs. The kind is determined by **where the index lives**, so an object and a manifest under the same key are two entries in two different packs and can never impersonate each other.

## Frames and encodings

Every entry begins with a **15-byte frame header**:

| Field     | Length | Meaning                            |
| --------- | ------ | ---------------------------------- |
| magic     | 4      | `RLST`                             |
| version   | 1      | Frame format version, currently 1  |
| codec     | 1      | How the payload is encoded         |
| layout    | 1      | The layout of the payload          |
| plain_len | 8      | Decoded payload length, big-endian |

The frame itself is **never compressed**, so a reader reads the frame first and thereby knows how to decompress the payload.

There are three codecs:

| Number | Codec | Purpose                 |
| ------ | ----- | ----------------------- |
| 0      | Raw   | Stored as-is            |
| 1      | Lz4   | Speed-first compression |
| 2      | Zstd  | Size-first compression  |

There are two layouts:

| Number | Layout  | What the payload is           |
| ------ | ------- | ----------------------------- |
| 0      | Single  | The content of a whole object |
| 1      | Chunked | The encoding of a manifest    |

`plain_len` is the decoded payload length: for a Single entry it is the length of the object itself, for a Chunked entry it is the length of the encoded manifest. A frame says only *how it is stored*, never *what is stored*, so storage never needs to know the difference between files and directories.

## Packing and locking

A loose object is *one file per object*. As content accumulates, storage can pack them together: the pack file itself is just a *sequence of entries*, each still preceded by its own frame; the pack's index `.idx` records each key's offset and length within the pack. The index begins with `RLPI`, its current version is 2, and the entries are *sorted by key*, so lookups are a binary search rather than a scan. An entry's length *includes its frame*, so the bytes read back from a pack are exactly the same as when they were loose.

A pack is written by *writing the index first, then the pack file*; removal happens in the opposite order. That way a run interrupted halfway leaves a `.pack` with no index, and no reader will mistake it for a pack.

The storage root holds a `lock` file. Packing and repacking *read and modify an entire pack*, so they can only be done through a guard holding the lock; a write that would overwrite another run's work cannot begin.

## How to choose encoding and chunking on write

A storage that is *not told otherwise* decides chunking in this order:

1. The chunking the caller forces. It overrides every other answer, because it is an explicit instruction.
2. An extension claimed by a policy. A claim is the only answer that knows the format **without reading the content**, and it is also the fastest and most predictable one.
3. Content beginning with the ZIP signature `PK\x03\x04`. It is chunked on member boundaries; see "About Chunk Objects".
4. Text. If the first 8000 bytes contain no zero byte and the length is at least 4 KiB, it is chunked on line endings.
5. Everything else is stored whole, unchunked.

The choice of codec is *separate* from chunking:

- ZIP containers use Raw, because compressing an already compressed container gains nothing.
- Text uses Zstd.
- Everything else uses the storage's configured codec, **Raw by default**.

The current version ships two format policies: `psd-basic` claims `.psd`, and `blend-basic` claims `.blend`, `.blend1` and `.blend2`. The policy list will keep growing; content already written is **unaffected by any new policy**, because the chunking was never recorded.

## Reading and verification

When reading content, storage first looks for a manifest under the same key:

- **Manifest present:** read each chunk in turn, check that each chunk's length matches what the manifest recorded, then verify the assembled whole against the key.
- **No manifest:** the key holds one complete object; read it directly as an object and verify it.

The manifest itself is *not covered by the key*. The key promises the content, not how the manifest is written, so even if every chunk is correct, the assembled whole must still be verified separately. If verification fails, a chunk is missing or the manifest cannot be read, storage falls back to a complete object under the same key; only when that object is also absent does it report the manifest's problem. This way one piece of content can have *both* a manifest and a complete object, and a read never loses the key just because one of them is broken.

Failures come in three kinds:

| Failure     | Meaning                                                         |
| ----------- | --------------------------------------------------------------- |
| `NotFound`  | There is no content under this key                              |
| `Malformed` | What is stored is not a frame or manifest this version can read |
| `Corrupt`   | The content retrieved does not match the key                    |

A transferring peer verifies every entry on receipt; local reads go through `read_object` and verify the same way. Before syncing you can also check a set of keys with `unreadable`: chunked content counts as "held" only when **every chunk reads back and the whole also verifies**.

## Exchanging content between two ends

Two storages of the same kind can carry content to each other. The exchange works *only by key*, never by file or path:

- Both sides first agree on the protocol identifier, then start asking for keys.
- Content moves as a whole; chunked content travels together with its manifest and all its chunks, and the receiving end lays it out however it likes.
- The exchange is driven by the same transport code, and the version control index implements the same set of interfaces, so index sync uses the same channel.

Thus the key is the *only* thing the two ends need to agree on. How a storage shards, packs, compresses, or even chunks is something they never need to share.
