# rola-sharp-2

## Introduction

C# bindings for the Rorolala C ABI, second generation: the generated `RorolalaBinding` surface plus the hand-written `Scoop` family and writers the Desktop program calls. It shares the C header and library with the first generation, and adds a read side that produces no side effects and a write side that calls the same `librorolala` workflows as the command line.

## Boundary

It owns the `[DllImport]` surface generated from the C header, the marshalling and release wrappers over it, the `Scoop` types that load a Workspace, a Vault or the user's own data, and the writers that call the C ABI's write operations. It does not own the header or the exported symbols: both come from the Rust tree through `build.rs`, so Rust has to be built first, since the generator reads `.cache/rs-target/<profile>/ffi_bindings/`. It does not own VCS behaviour: every write workflow lives in `librorolala`, and this assembly only marshals it.
