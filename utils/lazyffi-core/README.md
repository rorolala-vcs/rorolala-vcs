# rorolala-utils-lazyffi-core

## Introduction

The `#[lazyffi]` naming and shape rules that the attribute macro and the C header generator both need.

## Boundary

It owns those shared answers: the export name of an item, a type, a method or a part of an enum, which types have a repr-C sibling, and when a variant needs a companion struct. It does not expand a macro or print a header: `rorolala-utils-lazyffi-macros` generates the Rust and `rorolala-dev-bindgen` writes the C, and both depend on this crate rather than repeating it.
