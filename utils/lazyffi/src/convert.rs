//! Conversion between Rust types and their repr-C siblings.
//!
//! A type crossing the boundary either lets C hold one of its values, or it does
//! not:
//!
//! - a **transparent** type has exactly **one** repr-C sibling, and all four
//!   conversions name it:
//!
//!   ```text
//!   InputType::From == InputPtr::From == ReturnType::Target == ReturnPtr::Target
//!   ```
//!
//! - an **opaque** type — an exported `struct`, whose layout C is never told — has
//!   no value C could name, so it crosses through a pointer in every direction:
//!   [`InputType::From`] and [`ReturnType::Target`] are `*mut` handles, while
//!   [`InputPtr::From`] and [`ReturnPtr::Target`] are the handle type itself, which
//!   C spells only as an incomplete `struct` it must not complete.
//!
//! The four traits differ only in how the value travels:
//!
//! | Trait | Direction | Transport |
//! | --- | --- | --- |
//! | [`InputType`] | C → Rust | by value |
//! | [`InputPtr`] | C → Rust | through a pointer (both halves of a mutable pass) |
//! | [`ReturnType`] | Rust → C | by value |
//! | [`ReturnPtr`] | Rust → C | through an owning pointer |
//!
//! A `&mut` parameter is handled by reading through [`InputPtr::input_ptr`], calling
//! the function with `&mut`, then writing the value back through the same pointer with
//! [`InputPtr::write_ptr`] — so a transparent type needs no layout compatibility
//! between `Self` and its repr, and an opaque one is moved in and out of place.
//!
//! The input conversions are `unsafe`: the value comes from C, so the caller is
//! responsible for it being valid (a readable pointer, a live string, …).

/// Receives a value from C by value.
pub trait InputType {
    /// The repr-C sibling, or — for an opaque type — a borrowed `*mut` handle.
    type From;

    /// Builds `Self` from a repr passed by value.
    ///
    /// # Safety
    ///
    /// `input` must be a valid value of the repr — in particular, any pointer it
    /// carries must be usable for the conversion.
    unsafe fn input_type(input: Self::From) -> Self;
}

/// Passes a value across the boundary through a pointer, in both directions.
///
/// This is the by-pointer pass: a `&mut` parameter or receiver is read with
/// [`input_ptr`](InputPtr::input_ptr) and written back with
/// [`write_ptr`](InputPtr::write_ptr).
///
/// `From` is what C points at — the repr-C sibling for a transparent type, and the
/// handle itself for an opaque one, whose handle *is* its value.
pub trait InputPtr {
    /// The repr-C sibling: what C points at.
    type From;

    /// Takes `Self` by reading a repr through `input`.
    ///
    /// A transparent type copies its repr out; an opaque one moves the value out,
    /// leaving the storage behind `input` bitwise stale and not to be dropped.
    ///
    /// # Safety
    ///
    /// `input` must be valid, aligned and readable for `Self::From`, and must
    /// stay readable for the duration of the call.
    unsafe fn input_ptr(input: *mut Self::From) -> Self;

    /// Puts `self` back through `target`, replacing what is there.
    ///
    /// # Safety
    ///
    /// `target` must be valid, aligned and writable for `Self::From`. When it is
    /// the pointer `Self` was read from, that storage must not have been read or
    /// dropped in between.
    unsafe fn write_ptr(self, target: *mut Self::From);
}

/// Hands a value back to C by value.
pub trait ReturnType {
    /// The repr-C sibling, or — for an opaque type — an owning `*mut` handle.
    type Target;

    /// Converts `self` into the repr handed back to C.
    fn return_self(self) -> Self::Target;
}

/// Hands a value back to C through an owning pointer.
pub trait ReturnPtr {
    /// The repr-C sibling: the type C points at.
    type Target;

    /// Moves `self` into freshly allocated repr memory.
    ///
    /// The caller owns the result and is responsible for releasing it.
    fn return_ptr(self) -> *const Self::Target;
}
