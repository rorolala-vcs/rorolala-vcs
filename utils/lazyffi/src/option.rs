//! The nullable return: `Option<T>` crossing as a pointer that can be null.
//!
//! Not every absence is a failure, and a `Result` says the opposite — its `Err` side is
//! a case to switch on, so a caller would have to invent a failure that only ever means
//! "nothing". `Option<T>` says it directly instead: it crosses as whatever `T` already
//! crosses as, with the **null pointer** standing for `None`.
//!
//! That only works where `T` already crosses as an owning pointer, because null is only
//! a value there. An exported `struct` does — C holds a value of one by pointer already
//! — so `#[lazyffi]` implements [`Nullable`] for it. Nothing else does: a scalar and an
//! exported `enum` cross by value, so there is no null to spend, and a `String` or
//! `PathBuf` crosses as a `char *` that already spends null on a string it cannot carry.
//! `Option<i32>` and `Option<MyEnum>` therefore do not compile, and the header generator
//! reports the C side of the same rule rather than inventing a pointer.

use crate::convert::ReturnType;

/// A type that can be absent where C sees it.
///
/// An `Option<T>` over such a type crosses as `T`'s own repr, with null standing for
/// `None`. Implemented by `#[lazyffi]` for every exported `struct`, whose repr is the
/// owning pointer C already holds a value by.
pub trait Nullable: ReturnType {
    /// The repr value that stands for `None`.
    fn null() -> Self::Target;
}

impl<T: Nullable> ReturnType for Option<T> {
    type Target = T::Target;

    /// `Some` crosses as itself; `None` crosses as the null the repr reserves for it.
    fn return_self(self) -> Self::Target {
        self.map_or_else(T::null, T::return_self)
    }
}
