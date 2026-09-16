//! The one result type every fallible export hands back.
//!
//! `Result<T, E>` cannot cross as itself: C has to be told a concrete layout, and one
//! layout per `(T, E)` pair would mean a generated type per export — and, worse, a
//! `ReturnType` implementation per pair, which would collide the moment two exports
//! return the same one. So there is a single [`RorolalaResult`] instead, and everything
//! about it is fixed: the tag, the layout, and the rule for reading the payload.
//!
//! The payload is an owned `void *`, because one type has one layout and two arbitrary
//! payload types cannot share one. That puts the type back in the caller's hands, which
//! is why a result is read like this:
//!
//! 1. read [`RorolalaResult::tag`] to learn which side of the `Result` came back,
//! 2. cast [`RorolalaResult::payload`] to the type the header names for that side,
//! 3. release it with that type's own `free_*`.
//!
//! A payload is only ever something that has an owning pointer of its own: an exported
//! `struct` (whose repr is already a pointer), a `String` or `PathBuf` (a `char *`), an
//! exported `enum` (a value, so it is boxed), or `()` for the `Ok` that carries nothing
//! — which is the common `Result<(), E>`, where the `Ok` payload is simply null.
//!
//! A scalar has no owning pointer and is deliberately not a payload: `Result<bool, E>`
//! does not compile, rather than inventing an allocation and a release for a number.

use core::ffi::c_void;
use std::path::PathBuf;

use crate::convert::ReturnType;

/// Which side of a `Result` a [`RorolalaResult`] carries.
///
/// The discriminants are part of the C contract, and C reads them as
/// `RorolalaResult_Ok` and `RorolalaResult_Err`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RorolalaResultTag {
    /// The call succeeded, and the payload is its value.
    Ok = 0,
    /// The call failed, and the payload is the error.
    Err = 1,
}

/// What a fallible export hands back.
///
/// See the [module docs](self) for how the payload is read and released. It is read
/// only through the tag: the value is meaningful for [`RorolalaResultTag::Ok`], the
/// error for [`RorolalaResultTag::Err`], and it is null when there is nothing to
/// carry.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RorolalaResult {
    /// Which side of the `Result` this is.
    pub tag: RorolalaResultTag,
    /// The value or the error, owned by the caller; null when there is none.
    pub payload: *mut c_void,
}

impl RorolalaResult {
    /// A success carrying `payload`, which its owner is to cast back and release.
    #[must_use]
    pub const fn ok(payload: *mut c_void) -> Self {
        Self {
            tag: RorolalaResultTag::Ok,
            payload,
        }
    }

    /// A failure carrying `payload`, which its owner is to cast back and release.
    #[must_use]
    pub const fn error(payload: *mut c_void) -> Self {
        Self {
            tag: RorolalaResultTag::Err,
            payload,
        }
    }

    /// Whether this is the error side.
    ///
    /// # Safety
    ///
    /// Reading the payload is safe, but *interpreting* it is not: only the C caller,
    /// which knows the signature, can say what the pointer points at.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self.tag, RorolalaResultTag::Err)
    }
}

/// A type a [`RorolalaResult`] can carry.
///
/// Implemented by `#[lazyffi]` for every exported `struct` and `enum`, and by this
/// crate for `String`, `PathBuf` and `()`. The four conversions ([`InputType`],
/// [`ReturnType`], and the two pointer forms) say how a type crosses; this one says only
/// how it is handed over *as a result payload*, which is always an owned pointer,
/// whoever ends up releasing it.
///
/// [`InputType`]: crate::InputType
/// [`ReturnType`]: crate::ReturnType
pub trait ResultPayload: Sized {
    /// Hands the value over as the owned pointer the result carries.
    fn into_payload(self) -> *mut c_void;
}

/// Nothing to carry: the `Ok` of a `Result<(), E>`, which C reads as a null payload.
impl ResultPayload for () {
    fn into_payload(self) -> *mut c_void {
        core::ptr::null_mut()
    }
}

/// The `char *` a `String` already crosses as, which `free_string` releases.
impl ResultPayload for String {
    fn into_payload(self) -> *mut c_void {
        <Self as ReturnType>::return_self(self).cast()
    }
}

/// The `char *` a `PathBuf` already crosses as, which `free_string` releases.
impl ResultPayload for PathBuf {
    fn into_payload(self) -> *mut c_void {
        <Self as ReturnType>::return_self(self).cast()
    }
}

impl<T: ResultPayload, E: ResultPayload> ReturnType for Result<T, E> {
    type Target = RorolalaResult;

    /// One conversion per `Result<T, E>`, for every `T` and `E` that can be a payload
    /// — which is what a single fixed result type buys, and what a repr per pair could
    /// not have.
    fn return_self(self) -> Self::Target {
        match self {
            Ok(value) => RorolalaResult::ok(value.into_payload()),
            Err(error) => RorolalaResult::error(error.into_payload()),
        }
    }
}
