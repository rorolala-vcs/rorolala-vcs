# rola-sharp

## Introduction

The C# bindings to Rorolala's C ABI: the generated `RorolalaBinding` surface and the hand-written wrappers the Desktop program calls.

## Boundary

It owns the `[DllImport]` surface generated from the C header, and the marshalling and release wrappers over it (`RolaWorkspace`, `RolaLayouts`, `RolaLayoutEntries`, `RolaWorkspaceConfig`, `RolaStatus`, `RolaTreeStatus`, `RolaTreeMark`, `RolaBook`, `RolaIgnoreRules`, `RolaOwnership`, `RolaUser`, `RolaVault`, `RolaAccount`, `RolaMember`, `RolaKeyLocateRule`, `RolaPaths`, `RolaVariantGraph`, `RolaVersionGraph`, `RolaDesktopPty`). It does not own the header or the exports: both come from the Rust tree through `build.rs`, so Rust has to be built first, since the generator reads `.cache/rs-target/<profile>/ffi_bindings/`. It carries no VCS logic and decides no behaviour of its own.

# Obsolete

This assembly is frozen for the V2 migration. Do not add members, do not rename types, and do not
change behaviour. Bug fixes only. New work belongs in `modules/rola-sharp-2`, whose types are
`RolaSharp.V2` and `RolaSharp.V2.Write`. The migration plan is `__PLAN_ROLA_SHARP_V2_MIGRATE`.
