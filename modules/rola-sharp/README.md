# rola-sharp

The C# bindings to Rorolala's C ABI: `RolaSharp.dll`.

## What is here

`RorolalaBinding.cs` is generated, not written. Every build of this project runs
`rorolala-dev-rsharp-bindgen` over the C header the Rust side produces, and the file is rewritten —
so it is never stale against the header, and never edited by hand: an edit is lost with the next
build. It lists every export the header declares as a `[DllImport]` on
`RolaSharp.RorolalaBinding`, along with the enums and structs they name, and every member is
`internal`.

The public shape — the more OOP layer a caller is meant to use, which owns the marshalling and the
releases the raw surface leaves open — is hand-written over these bindings and is not written yet.
The assembly therefore has no public API at all.

## What it needs

The C header is the Rust side's output, under `.cache/rs-target/<profile>/ffi_bindings/`, so Rust has
to have been built first: a build of this project alone, before any Rust build, has nothing to read,
and the target says so rather than compiling a stale file. `./run.sh check` and `./run.sh export`
always build Rust first. The generator itself is a Rust program (`dev/rsharp-bindgen`), run by the
build through `cargo`.

## Where it goes

An export lays `RolaSharp.dll` in `build/lib/`, beside `rorolala.so` / `rorolala.dll`. The two are
one delivery: a consumer of the C ABI takes the library and the bindings together.
