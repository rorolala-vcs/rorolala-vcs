# rola-sharp

## Introduction

The C# bindings to Rorolala's C ABI: the generated `RorolalaBinding` surface and the hand-written wrappers the Desktop program calls.

## Boundary

It owns the `[DllImport]` surface generated from the C header, and the marshalling and release wrappers over it (`RolaWorkspace`, `RolaStatus`, `RolaBook`, `RolaIgnoreRules`, `RolaOwnership`, `RolaUser`, `RolaVariantGraph`, `RolaVersionGraph`, `RolaDesktopPty`). It does not own the header or the exports: both come from the Rust tree through `build.rs`, so Rust has to be built first, since the generator reads `.cache/rs-target/<profile>/ffi_bindings/`. It carries no VCS logic and decides no behaviour of its own.
