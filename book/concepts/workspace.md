---
title: "About a Workspace"
description: "What a Workspace is on disk, what its data directory holds, and how it relates to the working tree beside it."
keywords: workspace, client, working tree, layout, store, index, track, bring, cache, ignore, merging
---

# About a Workspace

A Workspace is the directory a person works in. It holds the files being edited and a data directory named `.rola`, and everything Rorolala knows about this piece of work is kept in that data directory rather than in the files themselves.

A directory is a Workspace because it holds `.rola/`. A run locates one by searching upwards from where it stands, so a command made in a subdirectory works on the Workspace above it.

## What a Workspace holds

| Path                            | What it is                                                                                                                                                 |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `.rola/workspace.toml`          | The configuration. It names the Vaults this Workspace knows, the one it reaches by default, the stores it binds, and the grace period its collection uses. |
| `.rola/auth/`                   | Keys kept for this Workspace.                                                                                                                              |
| `.rola/layout/`                 | The one Layout the work is recorded in. `TRACK` names the Vault whose Layout the work is compared with.                                                    |
| `.rola/index/`                  | The version index: variants, versions, creators and messages made here.                                                                                    |
| `.rola/storage/`                | The local store, see `vcs-knowledge/storage`.                                                                                                              |
| `.rola/cache/readonly-layouts/` | Read-only copies of Vault Layouts this Workspace has fetched.                                                                                              |
| `.rola/info/ignore/`            | The ignore rules this Workspace reads.                                                                                                                     |
| `.rola/lock`                    | The file a run takes while it changes the Workspace.                                                                                                       |
| `.rola/MERGING`                 | Present only while a merge is waiting. It records which variant file belongs to which path.                                                                |

On Windows the `.rola` directory is marked hidden, which is that platform's way of saying what a name beginning with a dot says elsewhere.

## The working tree and the Layout

The files beside `.rola` are the work a person sees. The Layout is what Rorolala records about that work: which path names which entry, the version that entry is at, and who holds it.

The two are read against each other, not kept in step by watching the filesystem:

- `rola status` reads the tree against the Layout and says what is lost, untagged, renamed or modified.
- `rola track` writes content into the store, makes a version, and renews the Layout entry for each path it records.
- `rola bring` puts an entry the Vault holds at a path here.
- `rola align` settles a path the Layout names and the tree no longer holds, and confirms a move.

A Workspace that has been edited but not tracked has a tree that says more than its Layout does. That is a normal state, and it is what `status` is for.

## The configuration

| Section     | What it decides                                                                                     |
| ----------- | --------------------------------------------------------------------------------------------------- |
| `[vaults]`  | The Vaults this Workspace knows, by name and address.                                               |
| `[default]` | The Vault reached when a command names none, and the ordered list of stores the work is written to. |
| `[gc]`      | The grace period its collection gives content that is no longer named.                              |

A store that is written to by several places is described in `vcs-knowledge/storage`. The order matters: a write goes to the first store in the list that is there, and a read tries each store in turn.

## Making a Workspace

```sh
rola create projects/hero
```

Or, in the directory that is to become one:

```sh
rola init
```

Making a Workspace makes the directory, `.rola/`, `.rola/index/`, `.rola/storage/`, `.rola/layout/` and the ignore rules, and writes `workspace.toml`. The Layout begins empty and tracks no Vault until something is recorded or a Vault is bound.

## What a Workspace is not

A Workspace is not a copy of the Vault. It holds what it has fetched, which may be less than the Vault holds, and what it has recorded locally, which the Vault may not have seen yet.

It is also not the working tree. A file that is in the tree is not in the Layout until it is tracked, and an entry in the Layout is not a copy of content until that content is in a store. The three are separate, and each command says which two it is comparing.

## Working without the Vault

What is already here stays readable. Content the Workspace has not fetched is not here, so a read that needs it asks the Vault at that moment. A command whose work is the exchange itself refuses when `--offline` is set rather than waiting for a connection.

Nothing is fetched in advance. A Workspace that has never reached a Vault still has its tree, its Layout and its store, and a Vault that is added later is fetched as the commands that need it run.
