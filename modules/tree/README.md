# rorolala-tree

## Introduction

A directory versioned as one file: the structure a tree version records, and the reading, staging, status and merge a tree needs to produce it.

## Boundary

It owns the canonically ordered structure object, the `.rolaignore` files a tree reads, the baseline copy kept beside it, its staging store, the scan that turns work into a structure, the path by path merge, and the status between those. It does not own the Layout that names the tree, which is `modules/layout`'s, or the index the version is written into, which is `modules/vcs`'s. The digest cache and diff it borrows are `modules/workspace-analyze`'s, and the commands that drive it live in `app/`.
