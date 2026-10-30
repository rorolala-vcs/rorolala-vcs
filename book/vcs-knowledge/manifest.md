---
title: "About Manifest Object"
description: "Describes the Manifest object used to describe file composition"
keywords: concept, storage, manifest, blake3, chunking, chunk, objects, object
---

# About Manifest Object

A manifest records *how* a chunked piece of content is composed: it lists the chunks in content order, and each chunk is a key plus a length.

When a piece of content is split into two or more chunks, the store writes a manifest for it. Content saved whole has **no manifest**; under its key there is just one complete object.

## Content

A manifest records the chunks *only in order*.

Each chunk records two things:

| Field | Meaning                                               |
| ----- | ----------------------------------------------------- |
| key   | the chunk's key in the store, a 32-byte BLAKE3 digest |
| len   | the chunk's length, in bytes                          |

The order of the chunks is the order in the content. When reading, the store reads chunks in this order, checks each chunk against this length, and concatenates the bytes in sequence. Beyond that, a manifest carries **no information whatsoever**.

`content_len` is the sum of all chunk lengths. It is computed with **saturating addition**: a manifest read back from bytes has already passed the length-limit check, and a manifest built chunk by chunk takes each length from real bytes, so the total normally cannot overflow; and if a caller does manage to construct an overflowing length, the saturated result neither panics in a debug build nor wraps around to a small number in a release build. Callers that need to distinguish the **true total** from **overflow** should use `checked_content_len`, which returns `None` when the total does not fit.

## Encoding

A manifest's bytes are just a chunk count followed by a run of fixed-length records, with **no extra magic or version number**:

```text
u64 chunk count (big-endian)
repeated chunk count times:
  u8[32] chunk key digest
  u64    chunk length (big-endian)
```

Each record is a fixed **40 bytes**: a 32-byte digest plus an 8-byte length.

Decoding is strict:

- The chunk count must fit in the platform's `usize`.
- Every record must be complete.
- No bytes may remain after all records are read.
- The sum of the lengths must not overflow, and must not exceed `isize::MAX`.

The last rule is *not* an estimate of disk capacity; it is a limit on whether the content can be put back together at all. The content ultimately goes into a `Vec<u8>`, and no `Vec` can exceed `isize::MAX` bytes. A manifest above that limit describes content this build cannot hold, even if the disk has room.

Decoding does **not** reserve memory according to the chunk count the manifest claims. That count is only what the manifest *says*, not bytes already read; a corrupt or malicious manifest can claim a huge chunk count, and reserving according to it would exhaust memory before a single chunk has been read. The reservation is capped at the number of records the remaining bytes can hold, and the *true* chunk count is verified by reading records one at a time.

## Why the splitting scheme is not recorded

A manifest describes *the result*, not the process. This makes splitting a purely write-time choice:

- A reader needs only the manifest, not the splitter that was used at the time.
- The splitting scheme can be tuned or replaced, and content already written remains readable.
- When two stores save the same content using *different* splits, the key is the same; their manifests differ, but each can reassemble the same content.

This is also the boundary in "the content is the key, the split is not": the key promises the *content*; the manifest is merely how that content is laid out in one particular store.

## Why it is stored separately

Manifests are stored under `manifest/`, separate from objects, rather than in the same directory as objects. The reason is **garbage collection**:

- A manifest states **which chunks are referenced**.
- Reading *only* the manifests is enough to know which chunks "someone wants", and which chunks no manifest mentions—orphans that are no longer referenced.
- If manifests were mixed in with objects, collection would have to read every object, first decide whether it is a manifest, and only then decide.

Likewise, the index of a manifest pack is also placed under `manifest/`, rather than alongside the manifest pack files. That way, "read out all manifests" means reading **one directory**, without opening every pack.

The same key may have both an object and a manifest at once: they are **two entries, in two places**, and reads cannot confuse them. A chunk may itself happen to be exactly some complete piece of content, so "object" and "manifest" are not two mutually exclusive kinds of content, but **two storage forms under the same key**.

## When it is written

When writing a piece of content:

- The split yields **two or more chunks**: each chunk is written as an object, then a manifest is written for the whole content. The manifest is itself framed, with its layout marked as `Chunked`, and is encoded the same way as content.
- The split yields **fewer than two chunks**: the whole content is written as a single object, and any existing manifest under the same key is deleted. One chunk does not constitute a split, and leaving a manifest behind would only describe a storage form that no longer exists.

When chunked content is written, if a complete object already exists under the same key, it is **left in place**. It may be exactly a chunk referenced by some manifest, or it may simply be content left over from an earlier whole write; deleting an object is more dangerous than deleting a manifest, because an object may be referenced by other manifests.

## Reading

When reading a piece of content, the manifest is **the first thing consulted**:

1. **No manifest** under the same key: the content is a single complete object; read and verify it directly.
2. **Manifest present**: read each chunk in order, check that each chunk's actual length equals the length recorded in the manifest, then concatenate.
3. Verify the concatenated content against the key **once more**.

Step 3 **cannot be skipped**. The manifest is not covered by the key: the key is the hash of the *complete content*, while the manifest is only a description of how to assemble it. A manifest can point at correct chunks but in the wrong order, or point at chunks belonging to other content, so **every chunk being correct does not mean the assembled content is what the key promised**.

On error, the store first tries the complete object under the same key:

- The manifest cannot be read: if the object is present, answer with the object; otherwise report the manifest's `Malformed`.
- A chunk is missing or its length does not match: if the object is present, answer with the object; otherwise report `Corrupt`.
- The assembled content fails verification: if the object is present, answer with the object; otherwise report `Corrupt`.

Only when **the object is also absent** does the manifest's problem become the final answer; an unreadable store or an object whose hash does not match says more about what has gone wrong with the storage than "the manifest is malformed" does.
