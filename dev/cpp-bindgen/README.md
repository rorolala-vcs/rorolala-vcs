# rororala-dev-cpp-bindgen

## Introduction

Generates the C++ headers that sit under `rola_plus_plus/gen` from the C header `rororala-dev-bindgen` writes, and compiles what it wrote before keeping it so a generated header is never handed out in a state the compiler rejects.

## Boundary

It owns the header parse and the mechanical half of the C++ surface: the aliases into `rola::raw`, the release policies and `owned<T>`, the `RorolalaResult` unwrapping, the error tag mapping, the ABI assertions and the path constants. It does not own the hand-written layer, which describes the object model (scopes, readings, writers), nor the C header itself, which is `rororala-dev-bindgen`.
