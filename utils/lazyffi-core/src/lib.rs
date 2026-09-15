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
/// ```ignore
/// assert_eq!(rorolala_utils_lazyffi_core::value_name("FFI_SMOKE_NAME"), "ffi_ffi_smoke_name");
/// ```
#[must_use]
pub fn value_name(rust_name: &str) -> String {
    format!("ffi_{}", snake_case!(rust_name.to_string()))
}

/// Export name of a type (`struct`, `enum`): `FFI<PascalCase>`.
///
/// ```ignore
/// assert_eq!(rorolala_utils_lazyffi_core::type_name("Foo"), "FFIFoo");
/// ```
#[must_use]
pub fn type_name(rust_name: &str) -> String {
    format!("FFI{}", pascal_case!(rust_name.to_string()))
}
