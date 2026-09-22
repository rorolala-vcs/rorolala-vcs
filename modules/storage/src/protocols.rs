//! The storage protocols: one implementation per kind of store.
//!
//! A protocol is what an object is laid out and encoded by, so a caller picks one, hands over
//! paths and keys, and never has to know how the bytes are kept.
//!
//! There is one so far: [`RorolalaStorage`], the store Rorolala keeps. A Workspace keeps one
//! inside its data directory and a Vault keeps one under its root; the same store also speaks
//! the protocol two of them move objects over.

mod rorolala;

pub use rorolala::*;
