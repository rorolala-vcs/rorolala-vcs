---
title: "About the Layout"
description: "The one mutable record in Rorolala: which path names which entry, what an entry holds, and how the record is kept."
keywords: layout, entry, uuid, path, version, owner, shard, snapshot, log, compaction, orphans
---

# About the Layout

A Layout is the record of what a piece of work is made of right now. It is the one thing in Rorolala that is not content-addressed and therefore not immutable: content is named by its hash and never changes, while a Layout says which content the work is at and who holds it.

A Workspace keeps one Layout under `.rola/layout`. A Vault keeps one under `layout`. The two are the same kind of record, and the difference is who is allowed to change which.

## What a Layout says

A Layout answers two questions:

- Which path names which `Uuid`.
- What is kept for each `Uuid`.

A path names one `Uuid`, and a `Uuid` is at one path. A move changes the path and keeps the `Uuid`, which is why a Layout can follow a file across a rename without treating it as a different file.

What a Layout keeps for one `Uuid` is the entry:

| Field | What it is |
| --- | --- |
| Kind | Whether the entry names a file or a tree. It is one kind for the entry's whole life. |
| Owner | The account that holds the entry, or nothing when no account does. |
| Version | The hash of the version the entry is at. |
| Description | What the entry says about itself. |
| Preference | The chunking policy its content is written with next time, by name. |

The preference is a name and nothing more. The Layout does not know what a policy is; the name is the one thing both sides agree on, and a name nothing answers to is refused where it is used rather than here.

The entry does not hold content. The version names a variant, and the variant names content in a store. A Layout that names a version whose content is not here is still a valid Layout; it is a read that fails, not a record that is wrong.

## How it is kept

A Layout is not one file. Changes are spread over sixteen shards, and the shard a change belongs to is decided by the `Uuid` it names. Everything about one `Uuid`, its path and what it holds, is in that one shard.

Each shard has two files:

- A log, which changes are appended to.
- A snapshot, which holds the shard's whole state so that most of the log can be dropped.

An append is a single write followed by a flush, so a change is on disk when the call returns. Nothing is locked across a write, and no writer waits for another except one appending to the same shard.

A compaction folds a shard's log into its snapshot and empties the log. A reader that had an offset into the log is aged: the log is a new age, and the reader starts again from its beginning. One shard is compacted at a time, and the others keep being written.

Reads are answered from memory. Opening a Layout reads every snapshot and replays every log, and a refresh replays what has been appended since. A reader that wants what another process has just written asks for a refresh; a reader that does not pays nothing.

## Entries without a path, paths without an entry

A layout can hold an entry no path names, and a path whose entry holds nothing. Both are normal between the two changes a create is made of, since one change puts the entry in and the next gives it its path.

One that never resolves is worth seeing, because it is a create that did not finish. The Layout reports both kinds as orphans rather than refusing to open.

## One Layout per place

A Workspace has one Layout, and the `TRACK` file inside it names the Vault whose Layout the work is compared with. A Vault has one Layout, and it is the record of what the Vault holds.

A Workspace also keeps read-only copies of Vault Layouts under `.rola/cache/readonly-layouts/`. Those copies are fetched, never written, and are what ownership is read from. See `concepts/ownership`.

## What changes a Layout

Only a write does:

- `rola track` records content and renews the entry for each path. A file whose content has not changed is left alone.
- `rola bring` puts an entry the Vault holds at a path here, or joins a variant into a file.
- `rola align` confirms that a path the Layout names is gone, or that a path moved.

Ownership is an entry field, but it changes at the Vault rather than in the local Layout. The local copy is refreshed from the Vault after a claim or a release.

A reading never changes anything. `rola status` reads the tree against the Layout, and `rola layout tree-diff` reports how they stand apart, but settling them is `track` or `align`.
