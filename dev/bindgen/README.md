# rorolala-dev-bindgen

## Introduction

Writes the C header for Rorolala's `#[lazyffi]` exports and raw `#[no_mangle]` symbols, reading the Rust sources directly, and is the build dependency the root `build.rs` runs to put `rorolala_ffi.h` in the target directory.

## Boundary

It owns what the header declares and how each Rust type is spelled in C, including the documentation lines it copies and the order the definitions appear in. It does not own the `#[lazyffi]` naming rules, which are `rorolala-utils-lazyffi-core`, nor the attribute macro that expands them, which is `rorolala-utils-lazyffi-macros`: the `export` argument is parsed in both places, so a new argument has to be taught to both parsers. It does not own the C# side of the same surface, which `rorolala-dev-rsharp-bindgen` generates from the header this tool writes.
