# rorolala-utils-lazyffi

## Introduction

FFI export for Rorolala: the `#[lazyffi]` attribute, the conversions between Rust types and their repr-C siblings, and the runtime helpers the generated code calls.

## Boundary

It owns the attribute surface, the conversions, and the `RorolalaResult` and null-pointer conventions a fallible or absent return crosses by. It does not own the C header, which `rorolala-dev-bindgen` generates, nor the naming and shape rules the macro and that generator share, which are `rorolala-utils-lazyffi-core`'s.
