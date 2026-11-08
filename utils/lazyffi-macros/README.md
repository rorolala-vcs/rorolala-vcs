# rorolala-utils-lazyffi-macros

## Introduction

The procedural macros behind `#[lazyffi]`, which turn an annotated Rust item into its repr-C sibling and its `extern "C"` wrappers.

## Boundary

It owns only the token generation. The conversions and the runtime helpers belong to `rorolala-utils-lazyffi`, which re-exports this crate, so a caller depends on that crate and never on this one. The naming rules it follows come from `rorolala-utils-lazyffi-core`.
