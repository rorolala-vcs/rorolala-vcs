#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use core::ffi::c_char;
use std::ffi::CString;

pub mod builtin;
mod convert;

pub use convert::*;
pub use rorolala_utils_lazyffi_macros::*;

/// Allocates a NUL-terminated copy of `s` for handing to C.
///
/// The caller owns the result and must release it with [`ffi_free_string`].
/// Returns a null pointer if `s` contains an interior NUL byte.
///
/// This is the building block used by `#[lazyffi]`-generated code; it is not
/// meant to be called directly.
#[doc(hidden)]
#[must_use]
pub fn __export_str(s: &str) -> *mut c_char {
    CString::new(s).map_or(core::ptr::null_mut(), CString::into_raw)
}

/// Releases a C string previously returned by a `#[lazyffi]` export.
///
/// # Safety
///
/// `ptr` must be null, or a pointer returned by a `#[lazyffi]` export that has
/// not already been freed. Passing any other pointer, or freeing the same
/// pointer twice, is undefined behaviour.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ffi_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }

    // SAFETY: the caller guarantees `ptr` came from `CString::into_raw` and has
    // not been freed yet.
    drop(unsafe { CString::from_raw(ptr) });
}
