# rorolala-ffi

## Introduction

`rorolala-ffi` is the C ABI of Rorolala: a `cdylib` and a `staticlib` that re-export the libraries declaring `#[lazyffi]` items, so their `#[unsafe(no_mangle)]` symbols reach `librorolala.so` and `librorolala.a`.

## Boundary

It owns the artifact name, `rorolala`, its two crate types, and the re-export that carries every symbol into the produced library. It does not own the exported surface, which is declared with `#[lazyffi]` under the root `src`, `modules` and `utils`, nor the header, which the root build script writes with `rorolala-dev-bindgen` into `ffi_bindings/` for each profile. It does not own the behaviour behind an export, which is `librorolala`. The release programs do not depend on this crate, which is why `tests/ffi-c` builds it itself.
