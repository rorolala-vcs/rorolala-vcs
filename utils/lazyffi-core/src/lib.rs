//! The rules `#[lazyffi]` follows, shared by the macro and the header generator.
//!
//! `lazyffi` exists to serve Rorolala, so the naming and repr rules are stated
//! once, here, instead of being repeated in `rorolala-utils-lazyffi-macros` and
//! `rorolala-dev-bindgen`. Those two would otherwise drift apart silently.

use just_fmt::{pascal_case, snake_case};

/// The repr-C sibling of [`String`].
///
/// `builtin` implements the conversions against exactly this type, and the header
/// generator spells it as `char *`.
pub const STRING_REPR: &str = "*mut c_char";

/// Name of the function that releases a string handed out by an export.
///
/// `rorolala-utils-lazyffi` defines it with `#[unsafe(no_mangle)]`, so this name
/// is a link-time contract; the header generator declares exactly this symbol,
/// since C cannot release an allocated string without it.
pub const FREE_STRING: &str = "free_string";

/// Name of the one result type every fallible export hands back.
///
/// Unlike the reprs generated per item, this one is fixed: it is declared once for
/// the whole surface, always, and a `Result<T, E>` return always comes back as it.
/// Its payload is an owned `void *` — the caller reads the tag to learn which of `T`
/// or `E` it is, casts to that type's own repr, and releases it with that type's own
/// `free_*`. The tag and its variants follow the same rules as any other exported
/// enum (see [`tag_type_name`] and [`c_variant_name`]), so the two sides cannot drift.
///
/// ```
/// assert_eq!(rorolala_utils_lazyffi_core::RESULT_REPR, "RorolalaResult");
/// assert_eq!(
///     rorolala_utils_lazyffi_core::c_variant_name(
///         rorolala_utils_lazyffi_core::RESULT_REPR,
///         rorolala_utils_lazyffi_core::RESULT_OK_VARIANT,
///     ),
///     "RorolalaResult_Ok",
/// );
/// ```
pub const RESULT_REPR: &str = "RorolalaResult";

/// Variant of [`RESULT_REPR`] carrying `Ok`'s payload.
pub const RESULT_OK_VARIANT: &str = "Ok";

/// Variant of [`RESULT_REPR`] carrying `Err`'s payload.
pub const RESULT_ERR_VARIANT: &str = "Err";

/// Invokes `$callback!` with the list of scalar types whose repr is themselves.
///
/// The `builtin` conversions are implemented for these types, and the header
/// generator maps them to C; both go through this macro, so the list cannot drift.
#[macro_export]
macro_rules! for_each_scalar {
    ($callback:ident) => {
        $callback!(
            bool, char, f32, f64, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
        );
    };
}

/// Export name of a value-level item (`fn`, `const`): `ffi_<snake_case>`.
///
/// ```
/// assert_eq!(rorolala_utils_lazyffi_core::value_name("FFI_SMOKE_NAME"), "ffi_ffi_smoke_name");
/// ```
#[must_use]
pub fn value_name(rust_name: &str) -> String {
    format!("ffi_{}", snake_case!(rust_name.to_string()))
}

/// Export name of a type (`struct`, `enum`): `FFI<PascalCase>`.
///
/// ```
/// assert_eq!(rorolala_utils_lazyffi_core::type_name("Foo"), "FFIFoo");
/// ```
#[must_use]
pub fn type_name(rust_name: &str) -> String {
    format!("FFI{}", pascal_case!(rust_name.to_string()))
}

/// Export name of a method inside an `impl`: `ffi_<snake_case(type)>_<snake_case(method)>`.
///
/// The type name is part of the export because `impl` blocks are flattened into
/// free functions: without it, `impl Foo { fn new }` and `impl Bar { fn new }`
/// would both claim the symbol `ffi_new`.
///
/// ```
/// assert_eq!(rorolala_utils_lazyffi_core::method_name("Vault", "open"), "ffi_vault_open");
/// ```
#[must_use]
pub fn method_name(rust_name: &str, method_name: &str) -> String {
    format!(
        "ffi_{}_{}",
        snake_case!(rust_name.to_string()),
        snake_case!(method_name.to_string())
    )
}

/// Name of the function that releases an owned value of a type: `free_<snake_case>`.
///
/// An exported `struct` is opaque to C, so every value of one that C receives is an
/// owning pointer, and each type needs a matching release — named the same way
/// [`FREE_STRING`] is, for the same reason. An exported `enum` gets one too, because a
/// `Result` payload that names it is boxed.
///
/// The name is the one C knows the item by: its `export` where it has one, and its
/// Rust name where it does not. An `export` therefore carries over to the release too,
/// so that two types a workspace keeps in different modules under one Rust name each
/// have a release of their own — where the Rust name alone would give both the same
/// one, and the two would be one symbol.
///
/// ```
/// assert_eq!(rorolala_utils_lazyffi_core::free_name("VaultConfig"), "free_vault_config");
/// assert_eq!(rorolala_utils_lazyffi_core::free_name("Counter"), "free_counter");
/// ```
#[must_use]
pub fn free_name(name: &str) -> String {
    format!("free_{}", snake_case!(name.to_string()))
}

/// Name of the tag enum generated for a data-carrying enum: `<repr>Tag`.
///
/// Takes the **repr** name rather than the Rust name, so an `export = ...`
/// override carries over to every generated sibling.
#[must_use]
pub fn tag_type_name(repr: &str) -> String {
    format!("{repr}Tag")
}

/// Name of the payload union generated for a data-carrying enum: `<repr>Payload`.
#[must_use]
pub fn payload_type_name(repr: &str) -> String {
    format!("{repr}Payload")
}

/// Name of the companion struct generated for a variant's payload:
/// `<repr><PascalCase(variant)>`.
#[must_use]
pub fn variant_type_name(repr: &str, variant_name: &str) -> String {
    format!("{repr}{}", pascal_case!(variant_name.to_string()))
}

/// C spelling of an enum enumerator: `<repr>_<PascalCase(variant)>`.
///
/// C has no scoped enumerators, so the variants of an exported enum are prefixed
/// with the name of the repr type to keep them out of each other's way.
#[must_use]
pub fn c_variant_name(repr: &str, variant_name: &str) -> String {
    format!("{repr}_{}", pascal_case!(variant_name.to_string()))
}

/// Field a unit variant occupies inside a payload union.
///
/// A union must be constructed through one of its fields, and a unit variant has
/// no payload to name, so every payload union carries this zero-sized slot. It
/// contributes nothing to the union's layout, so the header generator leaves it
/// out of the C union entirely.
pub const PAYLOAD_UNIT_FIELD: &str = "__unit";

/// The fields of an enum variant, as both the macro and the generator see them.
///
/// This mirrors `syn::Fields` without depending on `syn`, so the rule below stays
/// dependency-free yet stated exactly once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantFields {
    /// A unit variant.
    Unit,
    /// A tuple variant with this many fields.
    Unnamed(usize),
    /// A braced variant with this many fields.
    Named(usize),
}

/// Whether a variant's payload is wrapped in a companion struct.
///
/// A payload is wrapped when it has braced fields (so the field names survive
/// into C) or more than one tuple field (so the union only ever names one type
/// per variant). A single tuple field is used directly.
///
/// ```
/// assert!(!rorolala_utils_lazyffi_core::variant_needs_companion(
///     &rorolala_utils_lazyffi_core::VariantFields::Unnamed(1)
/// ));
/// assert!(rorolala_utils_lazyffi_core::variant_needs_companion(
///     &rorolala_utils_lazyffi_core::VariantFields::Named(1)
/// ));
/// ```
#[must_use]
pub fn variant_needs_companion(fields: &VariantFields) -> bool {
    match fields {
        VariantFields::Unit => false,
        VariantFields::Unnamed(count) => *count > 1,
        VariantFields::Named(_) => true,
    }
}
