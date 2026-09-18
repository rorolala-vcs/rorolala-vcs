# rorolala-protocol-macros

Procedural macros for the wire protocol.

This crate is an implementation detail of `rorolala-protocol`, which re-exports it.
Depend on `rorolala-protocol` instead of this crate.

The one macro here, `internal_repeat!`, is what makes an API whose shape is repeated
once per arity writable once: the body is written as a single template and emitted
again for every number in a range, with the placeholders inside rewritten per copy.
A `macro_rules!` cannot express it, because repeating exactly *N* times for a literal
`N` needs a counter, which declarative macros do not have.
