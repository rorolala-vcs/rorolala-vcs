# rorolala-utils-lazyffi-core

The `#[lazyffi]` rules shared by the attribute macro and the C-header generator.

Both sides need the same answers to the same questions — what is the export name of a
value-level item, of a type, of a method, of the parts of a data-carrying enum; which
types have a repr-C sibling built in; when a variant's payload needs a companion
struct — so those answers live here once. `rorolala-utils-lazyffi-macros` and
`rorolala-dev-bindgen` both depend on this crate rather than repeating them.

This crate deliberately has no dependency on `syn`: the one rule that is about syntax
shapes, whether a variant's payload is wrapped, is expressed over [`VariantFields`], a
three-case mirror of `syn::Fields` that both callers fill in.
