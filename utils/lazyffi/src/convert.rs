//! Conversion between Rust types and their repr-C siblings.
//!
//! Every type crossing the FFI boundary has exactly **one** repr-C sibling, used
//! in both directions:
//!
//! ```text
//! InputType::From == InputPtr::From == ReturnType::Target == ReturnPtr::Target
//! ```
//!
//! The four traits differ only in how the repr travels:
//!
//! | Trait | Direction | Transport |
//! | --- | --- | --- |
//! | [`InputType`] | C → Rust | by value |
//! | [`InputPtr`] | C → Rust | through a pointer (read half of a mutable pass) |
//! | [`ReturnType`] | Rust → C | by value |
//! | [`ReturnPtr`] | Rust → C | through an owning pointer |
//!
//! A `&mut` parameter is handled by reading through [`InputPtr`], calling the
//! function with `&mut`, then writing the value back through the same pointer
//! with [`ReturnType`] — the repr-C sibling is the single shape shared by both
//! halves, so no layout compatibility between `Self` and its repr is required.
//!
//! The two input traits are `unsafe`: the repr comes from C, so the caller is
//! responsible for it being a valid value (a readable pointer, a live string, …).

/// Receives a value from C by value.
pub trait InputType {
    /// The repr-C sibling.
    type From;

    /// Builds `Self` from a repr passed by value.
    ///
    /// # Safety
    ///
    /// `input` must be a valid value of the repr — in particular, any pointer it
    /// carries must be usable for the conversion.
    unsafe fn input_type(input: Self::From) -> Self;
}

/// Receives a value from C through a pointer.
///
/// This is the read half of a mutable pass: `&mut` parameters are copied in with
/// [`InputPtr`] and written back with [`ReturnType`].
pub trait InputPtr {
    /// The repr-C sibling.
    type From;

    /// Builds `Self` by reading a repr through `input`.
    ///
    /// # Safety
    ///
    /// `input` must be valid, aligned and readable for `Self::From`, and must
    /// stay readable for the duration of the call.
    unsafe fn input_ptr(input: *mut Self::From) -> Self;
}

/// Hands a value back to C by value.
pub trait ReturnType {
    /// The repr-C sibling.
    type Target;

    /// Converts `self` into the repr handed back to C.
    fn return_self(self) -> Self::Target;
}

/// Hands a value back to C through an owning pointer.
pub trait ReturnPtr {
    /// The repr-C sibling.
    type Target;

    /// Moves `self` into freshly allocated repr memory.
    ///
    /// The caller owns the result and is responsible for releasing it.
    fn return_ptr(self) -> *const Self::Target;
}
