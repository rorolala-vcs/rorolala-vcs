# rorolala-dev-bindgen

Generates the C header for Rorolala's `#[lazyffi]` surface.

A general-purpose generator (cbindgen) cannot render `#[lazyffi]` output, because the
generated signatures are written in terms of associated types
(`<T as InputType>::From`) which it has no way to resolve. This generator knows the
`#[lazyffi]` rules instead, so it works straight from the sources — no macro
expansion, and no nightly toolchain.

It is a build-dependency of the root package; see the `librorolala` repository root
`build.rs`.

## Design notes

### `export` is parsed twice

The `export = <Name>` override is parsed both here and in
`rorolala-utils-lazyffi-macros`. An attribute macro receives its item with the
`#[lazyffi]` attribute already stripped, so it can only read `export` from the
attribute's argument tokens; this generator reads the sources and sees the attribute
in place. The parsing shape therefore cannot be shared — only the defaults are, and
those come from `rorolala-utils-lazyffi-core`. **Adding a new attribute argument means
teaching both parsers about it.**
