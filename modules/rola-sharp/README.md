# rola-sharp

## Introduction

C# bindings for the Rorolala C ABI: the generated `RorolalaBinding` surface plus the hand-written `Scope` family and writers the Desktop program calls. The read side produces no side effects and the write side calls the same `librorolala` workflows as the command line.

## Boundary

It owns the `[DllImport]` surface generated from the C header, the marshalling and release wrappers over it, the `Scope` types that load a Workspace, a Vault or the user's own data, and the writers that call the C ABI's write operations. It does not own the header or the exported symbols: both come from the Rust tree through `build.rs`, so Rust has to be built first, since the generator reads `.cache/rs-target/<profile>/ffi_bindings/`. It does not own VCS behaviour: every write workflow lives in `librorolala`, and this assembly only marshals it.
