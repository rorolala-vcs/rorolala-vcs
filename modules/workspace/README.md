# rorolala-workspace

## Introduction

The client-side Workspace: the directory a person works in, with its configuration, store and index, the read-only copies it holds of Vault Layouts, and the merge records waiting in it.

## Boundary

It owns the `Workspace` type and locating one, `workspace.toml`, access to its store and index, the generation-based cache of fetched Vault Layout copies, the ownership lookup that reads such a copy, and the variant files a merge has waiting. It no longer owns status analysis: how the work stands is `modules/status`'s reading, and the tree diff behind it is `modules/workspace-analyze`'s. It does not own the Layout format (`modules/layout`) or the index objects (`modules/vcs`).
