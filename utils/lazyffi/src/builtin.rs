//! Conversions for common types.
//!
//! The traits are defined in the private `convert` module and re-exported at the
//! crate root, as [`crate::InputType`] and the three that go with it; this module
//! implements them for the primitives that most FFI signatures are built from.

use core::ffi::c_char;
use std::ffi::{CStr, OsStr};
use std::path::PathBuf;

use crate::convert::{InputPtr, InputRef, InputType, ReturnPtr, ReturnType};

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

            impl InputRef for $ty {
                type From = $ty;

                unsafe fn input_ref<'a>(input: *const Self::From) -> &'a Self {
                    // SAFETY: the caller guarantees a valid, aligned, readable pointer
                    // that stays readable for the duration of the borrow.
                    unsafe { &*input }
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
///   [`crate::free_string`].
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

/// The repr of [`PathBuf`] is a C string, exactly as [`String`]'s is.
///
/// A path is not text, though, so the bytes travel as the platform encodes them
/// ([`OsStr::as_encoded_bytes`]): a path Rust hands out and C gives straight back
/// comes back unchanged, including one that is not valid UTF-8. That holds wherever a
/// path *is* bytes, which is everywhere but Windows — there a C string is read as
/// UTF-8, lossily, exactly as [`String`] reads one, because the platform's encoding is
/// what makes a byte sequence a valid path. Ownership moves in the usual C direction:
///
/// - **input borrows**: the C string is copied and the caller keeps its buffer,
/// - **return allocates**: the caller owns the buffer and releases it with the very
///   same [`crate::free_string`] a [`String`] uses.
///
/// The by-pointer conversions are deliberately not implemented — a pointer to a
/// pointer is meaningless for a C string.
impl InputType for PathBuf {
    type From = *mut c_char;

    unsafe fn input_type(input: Self::From) -> Self {
        if input.is_null() {
            return Self::new();
        }

        // SAFETY: null was handled above; the caller guarantees a NUL-terminated
        // C string that stays alive for the duration of the call.
        let c_str = unsafe { CStr::from_ptr(input) };

        // On Unix a path *is* its bytes, so anything a C string can hold is a valid
        // path. Elsewhere the platform's encoding is what makes bytes valid, and a
        // C string is expected to be UTF-8 — so it is read the same way [`String`]
        // reads one, lossily, rather than assumed.
        #[cfg(unix)]
        // SAFETY: the bytes neither come from `as_encoded_bytes` on another platform
        // nor contain an interior NUL (a C string cannot), which is what makes them
        // valid encoded bytes on Unix.
        let os_str = unsafe { OsStr::from_encoded_bytes_unchecked(c_str.to_bytes()) };
        #[cfg(not(unix))]
        let os_str = OsStr::new(&*c_str.to_string_lossy());

        Self::from(os_str)
    }
}

impl ReturnType for PathBuf {
    type Target = *mut c_char;

    fn return_self(self) -> Self::Target {
        crate::__export_bytes(self.as_os_str().as_encoded_bytes())
    }
}
