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

```rust
use rorolala_utils_lazyffi::lazyffi;

/// A rectangle.
///
/// # FFI
/// Moved in and out by pointer; the handle owns its storage.
///
/// # Invariants
/// Never negative.
#[lazyffi]
pub struct Rect {}

fn main() {}
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

### Opaque types

An exported `struct` is emitted as an incomplete type — `typedef struct FFIVault
FFIVault;` — with no fields and no layout. C is told the type exists and nothing else,
so a value of one can only cross as a pointer:

| Rust | header |
| --- | --- |
| `&mut Vault` | `FFIVault *` |
| `Vault` (parameter) | `FFIVault *`, and the call takes ownership of it |
| `-> Vault` | `FFIVault *`, owned by the caller |
| `-> Self` in `impl Vault` | `FFIVault *`, owned by the caller |

Every opaque struct therefore also declares its release, `void
ffi_free_<type>(FFIVault *value);`, which is what frees the pointers the exports hand
out. This is also why a resource may hold values with no repr-C sibling at all — a
`PathBuf`, a socket: its fields are never converted, so nothing has to map them to C.

A by-value parameter is the one sharp edge: the pointer it is spelled as is C's own
storage, and the call moves the value out of it. C must treat the handle as given away
— not used again, and not released.

An `enum` is *not* opaque: its tag and payload are its interface, and C has to be able
to read them. A payload field whose type is opaque is spelled as a pointer, which is
the only way an incomplete type can appear inside another type.

### Strings and paths

`String` and `PathBuf` cross as C strings, and `&str` and `&Path` cross as the same
thing without the copy being visible in Rust:

| Rust | header |
| --- | --- |
| `&Path`, `&str` (or `PathBuf`, `String`) parameter | `const char *` |
| `-> PathBuf`, `-> String` | `char *`, owned by the caller |

A parameter is a position the callee only reads, so it is spelled `const` — which is
what lets a C++ caller pass a string literal. A returned string is owned and is
released with `ffi_free_string`, the same release a `String` uses.

A path is not text: its bytes travel as the platform encodes them
(`OsStr::as_encoded_bytes`), so a path Rust hands out and C gives straight back comes
back unchanged, including one that is not valid UTF-8. A null pointer on the way in
becomes an empty path or string; an interior NUL byte cannot be carried by a C string,
so a return that has one gives null instead of truncating silently.

`&mut String` and `&mut PathBuf` are rejected: a string has no second pointer to take.

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
