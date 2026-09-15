# rorolala-utils-lazyffi-core

The `#[lazyffi]` rules shared by the attribute macro and the C-header generator.

Both sides need the same answers to the same questions — what is the export name of a
value-level item, of a type; which types have a repr-C sibling built in — so those
answers live here once. `rorolala-utils-lazyffi-macros` and `rorolala-dev-bindgen`
both depend on this crate rather than repeating them.
