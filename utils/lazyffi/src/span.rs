//! A list of strings as C passes one: a pointer to the first and how many there are.
//!
//! C has no slice, so a `Vec<String>` a caller writes is handed over as a count beside a
//! pointer. [`StrSpan`] is the Rust side of that pair: it is what an exported struct
//! declares as a field type, and [`InputType`] copies every C string out of the span so
//! the caller keeps its buffers once the call returns.
//!
//! The repr is spelled out rather than generated, because it is a built-in shape: the
//! header generator declares `RolaStrSpan` itself, as `lazyffi-core` names it, and the
//! struct below must keep that exact layout.

use core::ffi::c_char;
use std::ffi::CStr;

use crate::convert::{InputRef, InputType, OwnedRef};

/// A list of strings as C holds one.
///
/// The strings belong to the caller and must outlive the call; nothing is released here.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RolaStrSpan {
    /// The strings, each a NUL-terminated C string.
    pub items: *const *const c_char,
    /// How many strings `items` points at.
    pub count: usize,
}

/// A list of strings received from C.
///
/// Reading one copies each string, because the caller's buffers are only promised for
/// the call. [`into_vec`](StrSpan::into_vec) hands the copies over.
#[derive(Clone, Debug)]
pub struct StrSpan {
    /// The strings, copied out of C's span.
    items: Vec<String>,
}

impl Default for StrSpan {
    /// No strings, which is what a caller that fills none passes.
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl StrSpan {
    /// Takes the copied strings.
    #[must_use]
    pub fn into_vec(self) -> Vec<String> {
        self.items
    }

    /// Reads the copied strings.
    #[must_use]
    pub fn as_slice(&self) -> &[String] {
        &self.items
    }
}

impl From<Vec<String>> for StrSpan {
    /// Takes strings a Rust caller already holds, for a request it fills itself.
    fn from(items: Vec<String>) -> Self {
        Self { items }
    }
}

impl InputType for StrSpan {
    type From = RolaStrSpan;

    // SAFETY: this impl obeys the trait's contract stated in `convert.rs`; every `CStr`
    // read it makes carries its own argument, a null entry having been answered first.
    unsafe fn input_type(input: Self::From) -> Self {
        // A span may be zeroed by a caller filling the field it sits in, and a null
        // pointer is not readable however small the count says it is.
        if input.count == 0 || input.items.is_null() {
            return Self { items: Vec::new() };
        }

        // SAFETY: null and zero count were handled above; the caller guarantees that
        // `items` points at `count` readable pointers that stay readable for the call.
        let pointers = unsafe { core::slice::from_raw_parts(input.items, input.count) };

        let items = pointers
            .iter()
            .map(|&pointer| {
                // A null entry carries no string. Reading it as the empty string keeps one
                // bad element from making the whole list unreadable, which is the same
                // answer a null `String` gets.
                if pointer.is_null() {
                    return String::new();
                }

                // SAFETY: the pointer is one of the entries the contract above says is a
                // NUL-terminated C string readable for the call.
                let c_str = unsafe { CStr::from_ptr(pointer) };

                c_str.to_string_lossy().into_owned()
            })
            .collect();

        Self { items }
    }
}

impl InputRef for StrSpan {
    type From = RolaStrSpan;

    type Ref<'a>
        = OwnedRef<Self>
    where
        Self: 'a;

    // SAFETY: this impl obeys the trait's contract stated in `convert.rs`; the read it
    // makes carries its own argument, and `InputType` turns the copy into owned strings.
    unsafe fn input_ref<'a>(input: *const Self::From) -> Self::Ref<'a> {
        // SAFETY: the caller guarantees a valid, aligned, readable pointer that stays
        // readable for the call; the repr holds no owning Rust value, so reading a copy
        // of it leaves C's storage untouched.
        let repr = unsafe { core::ptr::read(input) };

        // SAFETY: the repr was read as the trait's contract requires, so the copy it is
        // converted through reads only what the caller promised.
        OwnedRef::new(unsafe { <Self as InputType>::input_type(repr) })
    }
}
