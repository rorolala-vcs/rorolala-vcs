# rorolala-dev-bindgen

Generates the C header for Rorolala's `#[lazyffi]` surface.

A general-purpose generator (cbindgen) cannot render `#[lazyffi]` output, because the
generated signatures are written in terms of associated types
(`<T as InputType>::From`) which it has no way to resolve. This generator knows the
`#[lazyffi]` rules instead, so it works straight from the sources — no macro
expansion, and no nightly toolchain.

It is a build-dependency of the root package; see the `librorolala` repository root
`build.rs`. The header is written to `{target_dir}/{profile}/ffi_bindings/` as
`rorolala_ffi.h`, wrapped in `extern "C"` so it is usable from both C and C++.

## Design notes

### `export` is parsed twice

The `export = <Name>` override is parsed both here and in
`rorolala-utils-lazyffi-macros`, for items and for methods. An attribute macro
receives its item with the `#[lazyffi]` attribute already stripped, so it can only
read `export` from the attribute's argument tokens; this generator reads the sources
and sees the attribute in place. The parsing shape therefore cannot be shared — only
the defaults are, and those come from `rorolala-utils-lazyffi-core`. **Adding a new
attribute argument means teaching both parsers about it.**

The same holds for the shapes that are rejected: `&T` parameters, `&self`, `unsafe`
and `async` functions, generics, trait impls. The macro rejects them all, and so does
this generator, so the build stops with the diagnostic that names the construct rather
than after a plausible-looking declaration has been written.

### Definition order

C needs a type to be complete before it is used by value, and a data-carrying enum
expands into several interdependent definitions. The definitions are therefore emitted
in dependency order, computed from the C names each one mentions; a cycle, which Rust
cannot express by value either, is reported as an error instead of emitted.

### Documentation

Only two parts of an item's Rust docs reach the header: the first line, and the section
under a `# FFI` heading (the heading itself is dropped, and the section ends at the next
heading). Everything else stays on the Rust side, so the header stays readable no matter
how thorough the Rust docs are.

```rust,ignore
/// A rectangle.
///
/// # FFI
/// Passed by value; the fields are copied, not shared.
///
/// # Invariants
/// Never negative.
#[lazyffi]
pub struct Rect { /* ... */ }
```

becomes

```c
/**
 * A rectangle.
 *
 * Passed by value; the fields are copied, not shared.
 */
typedef struct FFIRect {
  /* ... */
} FFIRect;
```

Generated machinery (the tag enum, the payload union, the companion structs) carries no
comment of its own: what belongs to a variant or a field is documented on the member that
mirrors it, since that is where a C reader looks.

### Name resolution is by the last path segment

Types are matched by the final identifier of the path, so an export in one crate can
mention a type defined in a sibling crate: `auth::Token`, `rorolala_auth::Token` and a
bare `Token` all resolve to the same repr. Real name resolution would need a compiler,
and everything this generator cannot resolve is reported against the offending line
rather than skipped.

The consequences are worth knowing:

- an aliased import (`use rorolala_auth::Token as Seal;`) does not resolve;
- two types sharing a name are ambiguous, so two definitions that would claim the
  same C name are rejected outright instead of one silently shadowing the other.
