# rola-build

## Introduction

Produces everything this workspace ships, the library, the programs, the built-in book, the icons, the export layout and the Windows and Linux packages, and is reached as `cargo dev`.

## Boundary

It owns the build steps and what each artifact is made of. It does not own the checks over the tree, which are `rola-ci`, reached as `cargo ci`. It does not own the chunking measurement either: `cargo dev chunker` is a pass-through to `rorolala-dev-chunker-harness`, which prints its own options. The packaging assets under `dev/packaging/` are copied and configured here rather than written here.
