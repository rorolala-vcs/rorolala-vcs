# rorolala-dev-rsharp-bindgen

Generates the C# bindings for Rorolala's C ABI, from the C header the sibling generator
(`rorolala-dev-bindgen`) writes.

The input is the header, not the Rust sources: what a C# caller may name is exactly what the header
declares, so reading it is what keeps the two surfaces the same one.

It is run by a C# project rather than by the root build script: `RolaSharp` invokes this program on
every build, so its bindings are never stale against the header. That is why the header must exist
first — the Rust side has to have been built — and why a build of `RolaSharp` alone, before any Rust
build, has nothing to read.

## What is generated

- a free function becomes a `[DllImport]` extern on `RolaSharp.RorolalaBinding`;
- a named enum becomes a C# enum;
- a complete struct or union becomes a `[StructLayout]` struct, a union as an explicit layout;
- every pointer becomes a `nint`, because the header cannot say what a pointer points at and the
  layer above owns the marshalling — a returned string is a pointer to release with `free_string`;
- every member is `internal`, so the assembly's public shape is empty and the hand-written, more
  OOP layer is what a caller sees.

A name the header only forward-declares (an opaque type) is not emitted: it can only cross as a
pointer, and every pointer is a `nint`. A type held **by value** is looked up in the header, and an
opaque one there is refused, since C could not lay it out either.

## Why not read the sources

`rorolala-dev-bindgen` already resolves the `#[lazyffi]` surface, and doing it twice would be two
resolutions to keep in step. The header is the resolved surface, and it is stable enough to parse:
it is this project's own output, in a fixed style.

## Why no third-party C parser

The header is generated, so its grammar is known and small — `typedef enum`, `typedef struct`
(complete and forward), `typedef union`, and function declarations, in one fixed style. A general C
parser would be a large dependency for a shape this generator already controls.
