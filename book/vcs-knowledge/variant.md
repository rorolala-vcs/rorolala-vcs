---
title: "About Variant Object"
description: "Introduces the concept of variants in the version index"
keywords: concept, vcs-index, vcs, index, variant, objects, object
---

# About Variant Object

A variant is *the result of one edit*.

It records the **version it is based on** and the **actual content**, and is linked to the **creator** and **message** objects;

It is the **core concept** of the version control index, representing a new piece of stored content produced from a base version object.

> [!Note]
>
> Note: variants and versions are *fundamentally different*.
> By design, we treat a variant as new content and a version as acknowledged content.

## Fields

| Field              | Written to disk | Meaning                                |
| ------------------ | --------------- | -------------------------------------- |
| `storage_hash`     | Yes             | Hash of the storage entry              |
| `base_version`     | Yes             | Hash of the base version               |
| `join`             | Yes             | Hash of the joined variant; merge only |
| `creator`          | Yes             | Hash of the creator                    |
| `message`          | Yes             | Hash of the message                    |
| `base_version_num` | **No**          | Base version number, *derived*         |

> [!Note]
>
> `storage_hash` is the key into the content storage. It may be the key of a
> whole piece of content, or the key of a chunked piece of content;
>
> for details, see the `vcs-knowledge/variant` article.

## Variant Types

### Progressive Variant

A progressive variant is based on a version, points to a storage entry, and produces the next version:

```text
V2
  \
   R1A
  /
V1
```

The variant `R1A` is based on `V1`, and that variant is pointed to by the `V2` version.

### Merge Variant

A merge variant is likewise based on a version, but it also *joins* an existing variant and points to the content produced by both:

```text
V3
  \
   R2A
  /  \
V2    \
  \    \
   R1A  R1B
  /____/
V1
```

Version `V1` gives rise to two variants, `R1A` and `R1B`. `R1A` is pointed to by `V2`, while `R1B` is joined into `R2A` when `R2A` is created, and is then pointed to by `V3`.

When creating a merge variant, the base version, creator, and message are all **carried over** from the variant being invoked; only `join` is set to the hash of the joined variant, and `storage_hash` is set to the merged content.

> [!Note]
>
> Merging itself moves *neither* this text *nor* the base version.

## Variants vs. Versions

|                          | Variant                                                    | Version                        |
| ------------------------ | ---------------------------------------------------------- | ------------------------------ |
| Represents               | The result of one edit                                     | A fixed state of the edit      |
| Points to                | Base version, storage entry, optionally the joined variant | The variant that produced it   |
| Carries creator, message | Yes                                                        | **No**, carried by the variant |
| Number                   | Same as the base version it is based on                    | Computed from the chain        |

Fixing a variant only adds *one* version object. The creator, message, and base version do not change, so *"who did this and what was done"* is the same record both before and after the variant is fixed.

## Hashing and Bytes

A variant's hash is:

```text
BLAKE3("VRT" + storage_hash + base_version + join? + creator + message)
```

`VRT` is the salt for this kind of object. `join` only takes part in the hash when a merge variant is present; a progressive variant has **no such field** and does *not* use zero bytes as a placeholder. `base_version_num` does not take part, because it is *derived*.

Two variants are **equal if and only if** the fields written to disk are equal. A variant read back from the index with an unknown number, and a variant whose number has already been computed, are *the same variant*.

The bytes a variant writes:

```text
u8      kind, 0 for variant
u8[32]  storage_hash
u8[32]  base_version
u8      is merge: 0 or 1
u8[32]  join, only when the previous byte is 1
u8[32]  creator
u8[32]  message
```

A progressive variant is **130 bytes**, a merge variant **162 bytes**. The index stores it by its *semantic hash*: the key is the hash computed above, and the bytes written are the canonical encoding. When reading, the index decodes the object, rehashes the object, and compares the result with the key.

## Creator and Message

The creator and the message are **not free-text fields** but two separate index objects:

| Object    | Content              | Hash                   |
| --------- | -------------------- | ---------------------- |
| `Creator` | A name, UTF-8        | `BLAKE3("CRT" + name)` |
| `Message` | A description, UTF-8 | `BLAKE3("MSG" + text)` |

Both are at most **256 bytes**; anything longer or not valid UTF-8 is rejected. They store *text only*: `Creator` does not verify which account the name belongs to, and `Message` distinguishes neither subject from body. A variant stores only their hashes, so when the same message appears in several variants, the storage holds only *one copy* of the text.

## Unjoined Variant

A variant can be based on a version on the chain *without being joined by any version*. It is not on the trunk of the chain, nor beside any merge variant, so it does not appear when drawing the chain.

Such a variant still **belongs to this file**: as long as the version it is based on is on this chain, it is a *candidate edit* for this chain. The unjoined variant list of `lookback` lists them, ordered from the highest base version to the lowest. Join one of them and you get a merge variant; fix it, and you get the next version.

