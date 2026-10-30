---
title: "About Version Object"
description: "Introduces the concept of version objects in the version index"
keywords: concept, vcs-index, vcs, index, version, objects, object
---

# About Version Object

A **version** represents a *fixed* edit state—it is nothing more than a pointer to a *variant* object.

A version does not point directly to a file or to stored content. It points to a variant, and the variant in turn points to the version it is based on and to a storage entry. This forms a chain that can be walked back step by step.

## Fields

| Field         | Persisted | Meaning                                        |
| ------------- | --------- | ---------------------------------------------- |
| `variant`     | Yes       | Hash of the variant that produced this version |
| `version_num` | No        | Number of the version in its chain; *derived*  |

`version_num` is neither written to the index nor included in the hash. When a version is read, its number is *unknown*; wherever a number is needed (listing, drawing, stepping back one link), the index computes it by walking the chain.

## Hash and key

A version's hash is:

```text
BLAKE3("VER" + the variant's 32-byte digest)
```

`VER` is the salt for the *version* class of objects. The hash covers **only the variant**; it does not cover the version number, because the number is *derived*: the same variant is the same version no matter what number it is counted as.

The index stores versions under this hash, so a version's key is its **semantic hash**, not the hash of the bytes it writes to disk. Commands print and accept this hash in hexadecimal form, optionally with a `blake3:` prefix, or just the first **at least 4** characters.

The bytes a version writes are simple:

```text
u8      kind, 1 for a version
u8[32]  digest of the variant
```

33 bytes in total. When reading, the index decodes the object, recomputes the hash from the decoded object, and compares it with the key.

## Root version

Every chain starts from a fixed root. The root version is:

- Its variant hash is **32 zero bytes**.
- Its number is `u64::MAX`, that is, `-1` written as an unsigned number.

The root version is *the same object* in every index, so writing a root version into an index that already has one changes nothing. The first variant of any file is based on the root version, and walking back along the chain from any version will eventually reach it.

For a new repository, this version can be created with the `rola vcs-index create-rootver` command.

The root is not drawn into the version chain. It is the position "*before version 0*", so the first real version is numbered 0.

## Computing the version number

The number is computed by walking the chain backwards:

1. Starting from the version, read out the variant it points to.
2. From the variant, read out the version it is based on.
3. Repeat until the root is reached.
4. The number of links walked, minus one, is the number of the current version.

The chain of the first version is "version → variant → root": one step, minus one gives 0. The second version is two steps, giving 1, and so on.

Each step back reads one object, so the cost is **one index read per level**. This is suitable for computing the number **once and keeping it**, but not for calling repeatedly inside a loop. Reading an object does **not** compute the number automatically; `version_num` is only produced when the caller *explicitly* asks for it.

A variant's number is the number of the version it is based on. `variant_num` walks the *same* chain, only skipping the last link.
