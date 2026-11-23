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
//! - an **opaque** type (an exported `struct`, whose layout C is never told) has
//!   no value C could name, so it crosses through a pointer in every direction:
//!   [`InputType::From`] and [`ReturnType::Target`] are `*mut` handles, while
//!   [`InputPtr::From`], [`ReturnPtr::Target`] and [`InputRef::From`] are the handle
//!   type itself, which C spells only as an incomplete `struct` it must not complete.
//!
//! The traits differ only in how the value travels:
//!
//! | Trait | Direction | Transport |
//! | --- | --- | --- |
//! | [`InputType`] | C → Rust | by value |
//! | [`InputRef`] | C → Rust | borrowed in place, or built and held for the call |
//! | [`InputPtr`] | C → Rust | through a pointer (both halves of a mutable pass) |
//! | [`ReturnType`] | Rust → C | by value |
//! | [`ReturnPtr`] | Rust → C | through an owning pointer |
//!
//! A `&mut` parameter is handled by reading through [`InputPtr::input_ptr`], calling
//! the function with `&mut`, then writing the value back through the same pointer with
//! [`InputPtr::write_ptr`], so a transparent type needs no layout compatibility
//! between `Self` and its repr, and an opaque one is moved in and out of place.
//!
//! A `&` parameter is borrowed with [`InputRef::input_ref`] instead and never taken,
//! so the repr stays C's for the whole call. What that returns is not always a
//! reference: an opaque handle borrows where it lies, while a transparent input struct
//! has no such place, because its value is built out of C's fields. [`InputRef::Ref`]
//! is what lets both cross the same way, a borrow or an owned value the wrapper keeps
//! for the call and derefs to the callee's `&T`.
//!
//! The input conversions are `unsafe`: the value comes from C, so the caller is
//! responsible for it being valid (a readable pointer, a live string, and so on).

use core::ops::Deref;

/// Receives a value from C by value.
pub trait InputType {
    /// The repr-C sibling, or, for an opaque type, a borrowed `*mut` handle.
    type From;

    /// Builds `Self` from a repr passed by value.
    ///
    /// # Safety
    ///
    /// `input` must be a valid value of the repr, in particular, any pointer it
    /// carries must be usable for the conversion.
    unsafe fn input_type(input: Self::From) -> Self;
}

/// Borrows a value from C for the duration of the call.
///
/// This is how a `&T` parameter or a `&self` receiver crosses: C points at the repr
/// with a `const` pointer, and the borrow lasts for the call and no longer.
///
/// [`Ref`](InputRef::Ref) is what the wrapper actually holds. An opaque handle's repr
/// *is* its value in place, so it is borrowed there and nothing is taken. A transparent
/// input struct has no such place: its value is built out of C's fields, so the wrapper
/// owns it for the call and derefs to it. Both give the callee the `&T` it asked for.
pub trait InputRef {
    /// The repr-C sibling: what C points at.
    type From;

    /// What the wrapper holds: a borrow where the repr is the value in place, or the
    /// value built from the repr where it is not.
    type Ref<'a>: Deref<Target = Self>
    where
        Self: 'a;

    /// Borrows `Self` from the repr C owns, or builds it when there is nothing to borrow.
    ///
    /// # Safety
    ///
    /// `input` must be valid, aligned and readable for `Self::From`, and the storage
    /// must stay readable for as long as the function is called, which the generated
    /// wrapper limits to the call itself.
    unsafe fn input_ref<'a>(input: *const Self::From) -> Self::Ref<'a>;
}

/// An owned value standing in for a borrow.
///
/// [`InputRef::input_ref`] cannot return a reference to a value it had to build, so a
/// transparent input type hands the wrapper one of these instead. It derefs to the value
/// the callee sees, and lives no longer than the call.
pub struct OwnedRef<T>(T);

impl<T> OwnedRef<T> {
    /// Holds `value` for as long as this does.
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self(value)
    }
}

impl<T> Deref for OwnedRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Passes a value across the boundary through a pointer, in both directions.
///
/// This is the by-pointer pass: a `&mut` parameter or receiver is read with
/// [`input_ptr`](InputPtr::input_ptr) and written back with
/// [`write_ptr`](InputPtr::write_ptr).
///
/// `From` is what C points at, the repr-C sibling for a transparent type, and the
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
    /// The repr-C sibling, or, for an opaque type, an owning `*mut` handle.
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
