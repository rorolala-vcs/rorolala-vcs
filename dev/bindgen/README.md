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
| `&Vault` (or a `&self` receiver) | `const FFIVault *` |
| `&mut Vault` (or a `&mut self` receiver) | `FFIVault *` |
| `Vault` (parameter) | `FFIVault *`, and the call takes ownership of it |
| `-> Vault` | `FFIVault *`, owned by the caller |
| `-> Self` in `impl Vault` | `FFIVault *`, owned by the caller |

Every opaque struct therefore also declares its release, `void free_<type>(FFIVault
*value);`, which is what frees the pointers the exports hand out. This is also why a
resource may hold values with no repr-C sibling at all — a `PathBuf`, a socket: its
fields are never converted, so nothing has to map them to C.

A shared reference is `const` because Rust only reads through it, which is also what
lets a C++ caller pass something it holds as `const`. Two pointer positions are worth
keeping apart:

- a **by-value parameter** is the one sharp edge: the pointer it is spelled as is C's
own storage, and the call moves the value out of it. C must treat the handle as given
away — not used again, and not released;
- a **`&`** takes nothing and writes nothing back, so C keeps the handle and may borrow
it again — including from two calls at once.

An `enum` is *not* opaque: its tag and payload are its interface, and C has to be able
to read them. A payload field whose type is opaque is spelled as a pointer, which is
the only way an incomplete type can appear inside another type. That is also why an
`enum` has no `&` to spell: its repr is not the value in place, so a borrow of one
would have to point at a copy the wrapper built, which is not what the Rust signature
says. A scalar has one, since its repr *is* itself.

### Strings and paths

`String` and `PathBuf` cross as C strings, and `&str` and `&Path` cross as the same
thing without the copy being visible in Rust:

| Rust | header |
| --- | --- |
| `&Path`, `&str` (or `PathBuf`, `String`) parameter | `const char *` |
| `-> PathBuf`, `-> String` | `char *`, owned by the caller |

A parameter is a position the callee only reads, so it is spelled `const` — which is
what lets a C++ caller pass a string literal. A returned string is owned and is
released with `free_string`, the same release a `String` uses.

A path is not text: its bytes travel as the platform encodes them
(`OsStr::as_encoded_bytes`), so a path Rust hands out and C gives straight back comes
back unchanged, including one that is not valid UTF-8. A null pointer on the way in
becomes an empty path or string; an interior NUL byte cannot be carried by a C string,
so a return that has one gives null instead of truncating silently.

`&mut String` and `&mut PathBuf` are rejected: a string has no second pointer to take.

### Fallible returns

A `Result<T, E>` return is rendered as `RorolalaResult`, the one result type every header
declares — whether or not anything in it is fallible:

```c
typedef struct RorolalaResult {
  RorolalaResultTag tag;
  void * payload;
} RorolalaResult;
```

The payload is a `void *` because one layout cannot name two payload types, so what each
side holds is carried by the declaration's own doc block, which the generator appends to
the one the Rust source wrote:

```c
/**
 * Reads a counter.
 *
 * Returns a result:
 * - `Ok`: `char *`, owned; release it with `free_string`
 * - `Err`: `FFIRefusal *`, owned; release it with `free_refusal`
 */
RorolalaResult ffi_read(const FFICounter * counter);
```

A payload with no repr-C sibling is reported like any other unrenderable type, and so is
a scalar, which has no pointer of its own for C to cast or release. The `Ok` of a
`Result<(), E>` is named as carrying nothing.

An exported enum also declares a release now (`void free_<type>(FFIType *value);`),
because a payload naming one is boxed: nothing else in a signature needs to free a type
that crosses by value.

### Name resolution is by the last path segment

Types are matched by the final identifier of the path, so an export in one crate can
mention a type defined in a sibling crate: `auth::Token`, `rorolala_auth::Token` and a
bare `Token` all resolve to the same repr. Real name resolution would need a compiler,
and everything this generator cannot resolve is reported against the offending line
rather than skipped.

A name is a list of declarations, not one, and which is meant is decided by where it
is written: the nearest declaration wins. Two declarations equally near are ambiguous.
A **qualified** path breaks that tie, because the crate it names says which declaration
is meant — `rorolala_vault::Config` is the `Config` of the crate named `rorolala-vault`,
even when a sibling crate's `Config` sits equally near. The crate ident comes from the
`Cargo.toml` above the file that declares the type, so it is matched against a real
crate name rather than a guess. `crate::` and `self::` name the crate the reference is
written in; a `super::` path says nothing this generator can use, so it falls back to
the nearest declaration alone.

The consequences are worth knowing:

- an aliased import (`use rorolala_auth::Token as Seal;`) does not resolve;
- two types sharing a name are ambiguous, so two definitions that would claim the
  same C name are rejected outright instead of one silently shadowing the other.
