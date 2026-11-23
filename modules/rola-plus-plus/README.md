# rola-plus-plus

## Introduction

C++23 bindings for the Rorolala C ABI. The generated layer under `rola_plus_plus/gen` carries the release policies, the `Owned<T>` handles, the `RorolalaResult` unwrapping and the constants. The hand-written layer describes a Workspace or a Vault and the scopes each hands out (Layout, store, tree, VCS index, variant graph and status) with value types, and returns every failure as `std::expected`, so a read is a return value rather than a load followed by property access. A reading that copies everything out while its native handle is alive is a value; one whose handle must live on for later reads moves.

## Boundary

It owns the C++ object model of the read side: the scope descriptors, the readings they produce, the record types those readings hold and the umbrella header. It does not own the C header or the generated layer, which come from `rororala-dev-bindgen` and `dev/cpp-bindgen`. It does not own VCS behaviour: every read and write workflow lives in `librorolala`, and this library only marshals it. It does not wrap the `rola` command line, since it calls `librorolala` directly.
