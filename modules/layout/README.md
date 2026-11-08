# rorolala-layout

## Introduction

The Layout: which path names which `Uuid`, what each entry holds, and the append-only log a reader replays into memory.

## Boundary

It owns the sharded log, its snapshots and compaction, the normalized `LayoutPath`, and the per-entry data, owner and version included. It does not own the content a version names, which is `modules/storage`'s, or the versions and variants themselves, which are `modules/vcs`'s. How the work stands beside a Layout is `modules/status`'s question, and this crate answers only what the Layout says.
