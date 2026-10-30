---
title: "About Chunk Object"
description: "Introduces the Chunk object, the smallest unit of the storage system"
keywords: concept, storage, blake3, chunking, chunk, objects, object
---

# About Chunk Object

A chunk is the **smallest unit of stored content**.

## Splitting content

A splitter takes content and returns a set of ranges. The ranges are ordered, non-overlapping, and cover the whole content; concatenating the bytes they point to in order yields the original content.

Reading does not require knowing which splitter was used at write time. The manifest already gives the chunks and their order, so:

- The splitting method is not written into any stored object.
- Objects written yesterday do not need the splitter from that time to still exist in order to be read.
- New splitting methods can be added at any time, and existing content is unaffected.

Splitting **must be deterministic**. The same content must split at the same places every time; otherwise writing the same file twice produces two different manifests, and storage keeps two differently laid out copies of the content under the same key.

## General splitting methods

| Splitter | What it does                                 | Content it suits                                   |
| -------- | -------------------------------------------- | -------------------------------------------------- |
| `Whole`  | No splitting; the whole content is one chunk | Anything no other rule matches                     |
| `Fixed`  | One chunk per `size` bytes                   | Content written once and read whole                |
| `Cdc`    | Finds cut points from the content itself     | Large files edited repeatedly                      |
| `Text`   | Cuts at line ends, so chunks are whole lines | Text                                               |
| `Zip`    | Cuts at archive member boundaries            | `zip`, `docx`, `pptx`, `xlsx`, `odt`, `jar`, `apk` |
| `Policy` | Delegates to the format policy found by name | Formats with a dedicated policy, e.g. PSD, Blender |

### Whole

The whole content becomes one chunk. A single chunk does not constitute a *split*: it is still saved as a single object on write, and no manifest is produced.

### Fixed

Starting from the beginning, cut one chunk every `size` bytes; the last chunk may be shorter. This is the cheapest way to split, suited to content written once and then read whole. It is the *least* friendly to insertions: insert one byte in the middle of a file and every subsequent chunk shifts position, so each of those chunks becomes a new chunk that storage has never seen.

### Cdc

Cut points come from a rolling hash of the bytes, not from byte positions. On write, starting after the minimum chunk length, it rolls each byte's contribution; when the low bits are all zero, it cuts. The mask is the power of two minus one nearest to `average`, so cut points appear on average once every `average` bytes; `min` and `max` bound a chunk's minimum and maximum length.

The rolling hash depends only on byte values, so the same bytes always cut at the same place. Inserting or deleting a run of content only perturbs the chunk it falls into; the chunks before and after are still found at their original positions. *This is exactly where `Cdc` earns its cost and `Fixed` does not.*

`min`, `average`, and `max` are not written into any object. They can be tuned and the algorithm replaced wholesale, and existing manifests still read back.

### Text

Cut points are found the way `Cdc` finds them, then each is moved forward to the end of the line so that a chunk contains whole lines. Inserting, deleting, or changing lines is what edits actually move; the lines on either side of an edit are unchanged and so are their bytes, so they are still the same chunk—in this version, in another version, or even in another file.

A chunk may exceed `max` by up to one line, because a cut point landing mid-line is moved to the end of that line. When text reaches 4 KiB, storage uses line splitting by default; chunks range from about 1 KiB to 16 KiB, averaging 4 KiB. Small files are not split, because maintaining a manifest for them does not pay off.

### Zip

An archive is not one piece of content but many members packed in order. Finding cut points by content gains little here, because members are mostly compressor output, and the same content recompressed even once yields different bytes. Splitting at member boundaries gains a great deal: members the writer did not rewrite are still the same bytes in the same order, so storage already has that chunk, and changing one member adds one chunk rather than the whole archive.

The splitter reads the archive's central directory and takes each member, from its local header to the end of its data, as one range; after the last member come the central directory and the end record, combined into one range because their offsets shift as a whole when members change.

Nothing is decompressed along the way. Members are saved as their original bytes—header, name, and compressed data included—so what is read back is byte-for-byte what was written, and no zip bomb is opened just to learn the decompressed size. When the container structure cannot be read in a way the current version understands, the whole content is saved as one chunk; **that is always the right answer, whereas guessing wrong is not.**

### Policy

A format policy writes the splitting method for a container format as a type. Each policy declares three things:

- A name, in kebab-case, used by the command line and configuration to select the policy.
- The extensions it claims, lowercase and without dots, adopted automatically on write by extension.
- The splitting method itself.

The policy table is generated at build time from the individual policy files, so adding a policy means adding a file—no need to touch a shared list. Duplicate names or duplicate extensions **fail the build**: a name mapping to two splitting methods, or an extension claimed by two policies, would make storage's answer depend on table order.

The current version ships with `psd-basic` (claiming `psd`) and `blend-basic` (claiming `blend`, `blend1`, `blend2`).

When splitting by name, an unknown name means the whole content is saved in one piece. `split` returns ranges and cannot fail, so it is a total function over any name; the path that *actually selects* a policy looks up the table first and rejects unknown names there.

## How a chunk comes to be

When writing content, storage first selects a splitter as described above, then obtains the ranges:

- **Fewer than two ranges:** the whole content is written as one object. One chunk is not a split, so no manifest is written; any existing manifest under the same key is deleted, because it no longer describes this content.
- **Two or more ranges:** each range is written as an object holding that piece of content, all with the same encoding; then a manifest is written recording each chunk's key and length. The manifest is named with the same key and stored under `manifest/`.

Chunks are **content-addressed**. The same bytes appearing in another file, another version, or another format still get the same key, and storage keeps only one copy. A manifest remembers only *which chunk and how long*, not *which file it came from*.

Empty content likewise lands as an empty object, whose key is the hash of empty content.

