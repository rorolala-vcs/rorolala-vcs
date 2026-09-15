//! Conversions for common types.
//!
//! The traits are defined in the private `convert` module and re-exported at the
//! crate root, as [`crate::InputType`] and the three that go with it; this module
//! implements them for the primitives that most FFI signatures are built from.

use core::ffi::c_char;
use std::ffi::CStr;

use crate::convert::{InputPtr, InputType, ReturnPtr, ReturnType};

/// Implements the four conversion traits for a scalar type, whose repr is itself.
macro_rules! impl_scalar {
    ($($ty:ty),* $(,)?) => {
        $(
            impl InputType for $ty {
                type From = $ty;

                unsafe fn input_type(input: Self::From) -> Self {
                    input
                }
            }

            impl InputPtr for $ty {
                type From = $ty;

                unsafe fn input_ptr(input: *mut Self::From) -> Self {
                    // SAFETY: the caller guarantees a valid, aligned, readable pointer.
                    unsafe { *input }
                }

                unsafe fn write_ptr(self, target: *mut Self::From) {
                    // SAFETY: the caller guarantees a valid, aligned, writable pointer.
                    unsafe { *target = self };
                }
            }

            impl ReturnType for $ty {
                type Target = $ty;

                fn return_self(self) -> Self::Target {
                    self
                }
            }

            impl ReturnPtr for $ty {
                type Target = $ty;

                fn return_ptr(self) -> *const Self::Target {
                    Box::into_raw(Box::new(self))
                }
            }
        )*
    };
}

rorolala_utils_lazyffi_core::for_each_scalar!(impl_scalar);

/// The repr of [`String`] is a C string.
///
/// Ownership moves in the usual C direction:
///
/// - **input borrows**: the C string is copied and the caller keeps its buffer,
/// - **return allocates**: the caller owns the buffer and must release it with
///   [`crate::ffi_free_string`].
///
/// The by-pointer conversions are deliberately not implemented — a pointer to a
/// pointer is meaningless for a C string.
impl InputType for String {
    type From = *mut c_char;

    unsafe fn input_type(input: Self::From) -> Self {
        if input.is_null() {
            return Self::new();
        }

        // SAFETY: null was handled above; the caller guarantees a NUL-terminated
        // C string that stays alive for the duration of the call.
        let c_str = unsafe { CStr::from_ptr(input) };

        c_str.to_string_lossy().into_owned()
    }
}

impl ReturnType for String {
    type Target = *mut c_char;

    fn return_self(self) -> Self::Target {
        crate::__export_str(&self)
    }
}
