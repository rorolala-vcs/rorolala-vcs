# rorolala-chunking

## Introduction

The cuts that turn content into the pieces a store keeps, and the named policies that choose one from a container.

## Boundary

It owns the `Chunker` and `Policy` traits, the general cuts a store falls back on, the per-format strategies, and the registry its build script generates from them. It does not keep what it cuts: manifests, packs and keys are `modules/storage`'s, which re-exports this crate so a caller never names it. A stored entry records only the ordered chunk list, so a policy can be added or removed without any existing object becoming unreadable. Measuring a policy against its corpus is `dev/chunker-harness`, not this crate.
