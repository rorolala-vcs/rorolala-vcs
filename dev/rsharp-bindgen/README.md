# rorolala-dev-rsharp-bindgen

## Introduction

Generates the C# bindings for Rorolala's C ABI from the header `rorolala-dev-bindgen` writes, and runs inside the `RolaSharp` build so the two surfaces cannot drift apart.

## Boundary

It owns the header parse and the generated `internal` members: the externs on `RolaSharp.RorolalaBinding`, the enums, and the `[StructLayout]` structs. It does not own the header, which is `rorolala-dev-bindgen`, nor the hand-written, more OOP layer that a caller of `RolaSharp` sees. It does not marshal either: every pointer becomes a `nint` and the layer above owns the conversion, so a returned string is a pointer to release with `free_string`.
