#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use std::fmt;
use std::io;

use rorolala_utils_lazyffi::lazyffi;

/// What an I/O failure was, in `std`'s spelling.
///
/// One variant per [`io::ErrorKind`], and named after it, so nothing a caller could
/// branch on has to be read out of a message. `std` marks [`io::ErrorKind`] as
/// non-exhaustive, and gates a few of its variants behind unstable features, so this
/// stands for the kinds stable code can name: whatever is left over — a kind `std` adds
/// later, or one it does not let stable code spell — arrives as [`Other`](Self::Other).
#[lazyffi(export = RolaIoErrorKind)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IoErrorKind {
    /// An entity was not found, often a file.
    NotFound,
    /// The operation lacked the necessary privileges to complete.
    PermissionDenied,
    /// The connection was refused by the remote server.
    ConnectionRefused,
    /// The connection was reset by the remote server.
    ConnectionReset,
    /// The remote host is not reachable.
    HostUnreachable,
    /// The network is not reachable.
    NetworkUnreachable,
    /// The connection was aborted by the remote server.
    ConnectionAborted,
    /// The connection has not been established yet.
    NotConnected,
    /// The address is in use.
    AddrInUse,
    /// The address is not available.
    AddrNotAvailable,
    /// The network is down.
    NetworkDown,
    /// The pipe was closed, usually by writing to one whose read end is gone.
    BrokenPipe,
    /// An entity already exists, often a file.
    AlreadyExists,
    /// The operation would have to block, and was asked not to.
    WouldBlock,
    /// A filesystem object is, unexpectedly, not a directory.
    NotADirectory,
    /// A filesystem object is, unexpectedly, a directory.
    IsADirectory,
    /// A directory is not empty.
    DirectoryNotEmpty,
    /// The filesystem is read-only.
    ReadOnlyFilesystem,
    /// The network file handle the operation was given is stale.
    StaleNetworkFileHandle,
    /// A parameter was incorrect.
    InvalidInput,
    /// Data that was not valid for the operation was received.
    InvalidData,
    /// An operation timed out.
    TimedOut,
    /// A write returned zero when more was expected.
    WriteZero,
    /// The filesystem is full.
    StorageFull,
    /// The stream cannot be seeked.
    NotSeekable,
    /// A filesystem quota was exceeded.
    QuotaExceeded,
    /// The file is larger than the filesystem can hold.
    FileTooLarge,
    /// A resource is busy.
    ResourceBusy,
    /// An executable file is busy, usually because it is being written to.
    ExecutableFileBusy,
    /// An operation would deadlock.
    Deadlock,
    /// A move or copy between filesystems was attempted.
    CrossesDevices,
    /// A file has too many hard links.
    TooManyLinks,
    /// A filename is invalid.
    InvalidFilename,
    /// An argument list was longer than the limit allows.
    ArgumentListTooLong,
    /// The operation was interrupted before it completed, and may be retried.
    Interrupted,
    /// The operation is not supported.
    Unsupported,
    /// An end of file was reached when it was not expected.
    UnexpectedEof,
    /// An operation ran out of memory.
    OutOfMemory,
    /// A failure that falls under no other kind: what `std` does not classify, and
    /// whatever its non-exhaustive list grows next.
    #[default]
    Other,
}

impl From<io::ErrorKind> for IoErrorKind {
    fn from(kind: io::ErrorKind) -> Self {
        match kind {
            io::ErrorKind::NotFound => Self::NotFound,
            io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            io::ErrorKind::ConnectionRefused => Self::ConnectionRefused,
            io::ErrorKind::ConnectionReset => Self::ConnectionReset,
            io::ErrorKind::HostUnreachable => Self::HostUnreachable,
            io::ErrorKind::NetworkUnreachable => Self::NetworkUnreachable,
            io::ErrorKind::ConnectionAborted => Self::ConnectionAborted,
            io::ErrorKind::NotConnected => Self::NotConnected,
            io::ErrorKind::AddrInUse => Self::AddrInUse,
            io::ErrorKind::AddrNotAvailable => Self::AddrNotAvailable,
            io::ErrorKind::NetworkDown => Self::NetworkDown,
            io::ErrorKind::BrokenPipe => Self::BrokenPipe,
            io::ErrorKind::AlreadyExists => Self::AlreadyExists,
            io::ErrorKind::WouldBlock => Self::WouldBlock,
            io::ErrorKind::NotADirectory => Self::NotADirectory,
            io::ErrorKind::IsADirectory => Self::IsADirectory,
            io::ErrorKind::DirectoryNotEmpty => Self::DirectoryNotEmpty,
            io::ErrorKind::ReadOnlyFilesystem => Self::ReadOnlyFilesystem,
            io::ErrorKind::StaleNetworkFileHandle => Self::StaleNetworkFileHandle,
            io::ErrorKind::InvalidInput => Self::InvalidInput,
            io::ErrorKind::InvalidData => Self::InvalidData,
            io::ErrorKind::TimedOut => Self::TimedOut,
            io::ErrorKind::WriteZero => Self::WriteZero,
            io::ErrorKind::StorageFull => Self::StorageFull,
            io::ErrorKind::NotSeekable => Self::NotSeekable,
            io::ErrorKind::QuotaExceeded => Self::QuotaExceeded,
            io::ErrorKind::FileTooLarge => Self::FileTooLarge,
            io::ErrorKind::ResourceBusy => Self::ResourceBusy,
            io::ErrorKind::ExecutableFileBusy => Self::ExecutableFileBusy,
            io::ErrorKind::Deadlock => Self::Deadlock,
            io::ErrorKind::CrossesDevices => Self::CrossesDevices,
            io::ErrorKind::TooManyLinks => Self::TooManyLinks,
            io::ErrorKind::InvalidFilename => Self::InvalidFilename,
            io::ErrorKind::ArgumentListTooLong => Self::ArgumentListTooLong,
            io::ErrorKind::Interrupted => Self::Interrupted,
            io::ErrorKind::Unsupported => Self::Unsupported,
            io::ErrorKind::UnexpectedEof => Self::UnexpectedEof,
            io::ErrorKind::OutOfMemory => Self::OutOfMemory,
            _ => Self::Other,
        }
    }
}

impl From<IoErrorKind> for io::ErrorKind {
    fn from(kind: IoErrorKind) -> Self {
        match kind {
            IoErrorKind::NotFound => Self::NotFound,
            IoErrorKind::PermissionDenied => Self::PermissionDenied,
            IoErrorKind::ConnectionRefused => Self::ConnectionRefused,
            IoErrorKind::ConnectionReset => Self::ConnectionReset,
            IoErrorKind::HostUnreachable => Self::HostUnreachable,
            IoErrorKind::NetworkUnreachable => Self::NetworkUnreachable,
            IoErrorKind::ConnectionAborted => Self::ConnectionAborted,
            IoErrorKind::NotConnected => Self::NotConnected,
            IoErrorKind::AddrInUse => Self::AddrInUse,
            IoErrorKind::AddrNotAvailable => Self::AddrNotAvailable,
            IoErrorKind::NetworkDown => Self::NetworkDown,
            IoErrorKind::BrokenPipe => Self::BrokenPipe,
            IoErrorKind::AlreadyExists => Self::AlreadyExists,
            IoErrorKind::WouldBlock => Self::WouldBlock,
            IoErrorKind::NotADirectory => Self::NotADirectory,
            IoErrorKind::IsADirectory => Self::IsADirectory,
            IoErrorKind::DirectoryNotEmpty => Self::DirectoryNotEmpty,
            IoErrorKind::ReadOnlyFilesystem => Self::ReadOnlyFilesystem,
            IoErrorKind::StaleNetworkFileHandle => Self::StaleNetworkFileHandle,
            IoErrorKind::InvalidInput => Self::InvalidInput,
            IoErrorKind::InvalidData => Self::InvalidData,
            IoErrorKind::TimedOut => Self::TimedOut,
            IoErrorKind::WriteZero => Self::WriteZero,
            IoErrorKind::StorageFull => Self::StorageFull,
            IoErrorKind::NotSeekable => Self::NotSeekable,
            IoErrorKind::QuotaExceeded => Self::QuotaExceeded,
            IoErrorKind::FileTooLarge => Self::FileTooLarge,
            IoErrorKind::ResourceBusy => Self::ResourceBusy,
            IoErrorKind::ExecutableFileBusy => Self::ExecutableFileBusy,
            IoErrorKind::Deadlock => Self::Deadlock,
            IoErrorKind::CrossesDevices => Self::CrossesDevices,
            IoErrorKind::TooManyLinks => Self::TooManyLinks,
            IoErrorKind::InvalidFilename => Self::InvalidFilename,
            IoErrorKind::ArgumentListTooLong => Self::ArgumentListTooLong,
            IoErrorKind::Interrupted => Self::Interrupted,
            IoErrorKind::Unsupported => Self::Unsupported,
            IoErrorKind::UnexpectedEof => Self::UnexpectedEof,
            IoErrorKind::OutOfMemory => Self::OutOfMemory,
            IoErrorKind::Other => Self::Other,
        }
    }
}

/// An I/O failure, as it crosses the C ABI.
///
/// [`io::Error`] cannot be exported: it is not this workspace's type, so it can carry no
/// `#[lazyffi]`, and it has no repr-C sibling for an exported signature to name. What
/// crosses is the kind it was, as [`IoErrorKind`], together with what it said, which is
/// what a caller reads and branches on. Converting back gives an [`io::Error::new`] of
/// that kind carrying that message.
#[lazyffi(export = RolaIoError)]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct IoError {
    /// The kind of failure this was.
    kind: IoErrorKind,
    /// What the failure said.
    message: String,
}

impl IoError {
    /// Names a failure by its kind and what it said.
    #[must_use]
    pub const fn new(kind: IoErrorKind, message: String) -> Self {
        Self { kind, message }
    }
}

#[lazyffi]
impl IoError {
    /// The kind of failure this was.
    #[must_use]
    pub const fn kind(&self) -> IoErrorKind {
        self.kind
    }

    /// What the failure said.
    #[must_use]
    pub fn message(&self) -> String {
        self.message.clone()
    }
}

impl fmt::Display for IoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for IoError {}

impl From<io::Error> for IoError {
    fn from(source: io::Error) -> Self {
        Self::new(source.kind().into(), source.to_string())
    }
}

impl From<IoError> for io::Error {
    fn from(source: IoError) -> Self {
        Self::new(source.kind.into(), source.message)
    }
}

/// A failure to encode or decode a value, as it crosses the C ABI.
///
/// As [`IoError`], [`bincode2::Error`] is foreign and has no repr-C sibling, so what
/// crosses is the message it carried. It is also not a type of its own — it is
/// `Box<ErrorKind>` — so going back cannot be a [`From`] impl: neither the box nor the
/// kind is this crate's to implement [`From`] for. It is
/// [`into_bincode`](BincodeError::into_bincode) instead, which builds the
/// [`Custom`](bincode2::ErrorKind::Custom) kind out of the message.
#[lazyffi(export = RolaBincodeError)]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BincodeError {
    /// What the failure said.
    message: String,
}

impl BincodeError {
    /// Names a failure by what it said.
    #[must_use]
    pub const fn new(message: String) -> Self {
        Self { message }
    }

    /// The failure as `bincode2` spells it: a [`Custom`](bincode2::ErrorKind::Custom)
    /// kind carrying the message.
    #[must_use]
    pub fn into_bincode(self) -> bincode2::Error {
        Box::new(bincode2::ErrorKind::Custom(self.message))
    }
}

#[lazyffi]
impl BincodeError {
    /// What the failure said.
    #[must_use]
    pub fn message(&self) -> String {
        self.message.clone()
    }
}

impl fmt::Display for BincodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for BincodeError {}

impl From<bincode2::Error> for BincodeError {
    fn from(source: bincode2::Error) -> Self {
        Self::new(source.to_string())
    }
}

/// A failure to turn a value into JSON, as it crosses the C ABI.
///
/// As [`BincodeError`], `serde_json`'s error is foreign and has no repr-C sibling, so
/// what crosses is what it said *and where* — a line and a column, which is what a
/// caller needs to find the value that would not encode. Going back is
/// [`into_json`](JsonError::into_json): `serde_json` hands out no way to rebuild the
/// syntax error it describes, so what comes back is an I/O one carrying the message.
#[lazyffi(export = RolaJsonError)]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct JsonError {
    /// What the failure said.
    message: String,
    /// The line it was found on.
    line: usize,
    /// The column it was found on.
    column: usize,
}

impl JsonError {
    /// Names a failure by what it said and where it said it.
    #[must_use]
    pub const fn new(message: String, line: usize, column: usize) -> Self {
        Self {
            message,
            line,
            column,
        }
    }

    /// The failure as `serde_json` spells it, as far as one can be built: an I/O error
    /// carrying the message.
    #[must_use]
    pub fn into_json(self) -> serde_json::Error {
        serde_json::Error::io(io::Error::other(self.message))
    }
}

#[lazyffi]
impl JsonError {
    /// What the failure said.
    #[must_use]
    pub fn message(&self) -> String {
        self.message.clone()
    }

    /// The line it was found on.
    #[must_use]
    pub const fn line(&self) -> usize {
        self.line
    }

    /// The column it was found on.
    #[must_use]
    pub const fn column(&self) -> usize {
        self.column
    }
}

impl fmt::Display for JsonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for JsonError {}

/// A target that would not read as an address, as it crosses the C ABI.
///
/// [`AddrParseError`](std::net::AddrParseError) is foreign, carries nothing, and says the
/// same sentence whatever was wrong with the address, so what crosses is the target
/// itself: it is the one thing that says which address to go and look at. There is no
/// conversion back, for the reason [`JsonError`] has none either — `std` hands out no way
/// to build the error it describes.
#[lazyffi(export = RolaAddrError)]
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AddrError {
    /// The target that would not read as an address.
    target: String,
}

impl AddrError {
    /// Names the target that would not read as an address.
    #[must_use]
    pub const fn new(target: String) -> Self {
        Self { target }
    }
}

#[lazyffi]
impl AddrError {
    /// The target that would not read as an address.
    #[must_use]
    pub fn target(&self) -> String {
        self.target.clone()
    }
}

impl fmt::Display for AddrError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "`{}` is not an address", self.target)
    }
}

impl std::error::Error for AddrError {}

impl From<serde_json::Error> for JsonError {
    fn from(source: serde_json::Error) -> Self {
        Self::new(source.to_string(), source.line(), source.column())
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;
    use std::io::{Error as IoFailure, ErrorKind as StdKind};

    use super::{AddrError, BincodeError, IoError, IoErrorKind, JsonError};

    /// Every kind this crate names, paired with the `std` kind it stands for.
    ///
    /// Kept as one table so both directions are checked over the whole list at once: an arm
    /// that was swapped, dropped, or pointed at the wrong sibling shows up as a mismatch.
    /// `Other` is deliberately left out — it is where anything *not* listed lands, not a
    /// kind `std` names on the way in.
    const NAMED_KINDS: [(IoErrorKind, StdKind); 38] = [
        (IoErrorKind::NotFound, StdKind::NotFound),
        (IoErrorKind::PermissionDenied, StdKind::PermissionDenied),
        (IoErrorKind::ConnectionRefused, StdKind::ConnectionRefused),
        (IoErrorKind::ConnectionReset, StdKind::ConnectionReset),
        (IoErrorKind::HostUnreachable, StdKind::HostUnreachable),
        (IoErrorKind::NetworkUnreachable, StdKind::NetworkUnreachable),
        (IoErrorKind::ConnectionAborted, StdKind::ConnectionAborted),
        (IoErrorKind::NotConnected, StdKind::NotConnected),
        (IoErrorKind::AddrInUse, StdKind::AddrInUse),
        (IoErrorKind::AddrNotAvailable, StdKind::AddrNotAvailable),
        (IoErrorKind::NetworkDown, StdKind::NetworkDown),
        (IoErrorKind::BrokenPipe, StdKind::BrokenPipe),
        (IoErrorKind::AlreadyExists, StdKind::AlreadyExists),
        (IoErrorKind::WouldBlock, StdKind::WouldBlock),
        (IoErrorKind::NotADirectory, StdKind::NotADirectory),
        (IoErrorKind::IsADirectory, StdKind::IsADirectory),
        (IoErrorKind::DirectoryNotEmpty, StdKind::DirectoryNotEmpty),
        (IoErrorKind::ReadOnlyFilesystem, StdKind::ReadOnlyFilesystem),
        (
            IoErrorKind::StaleNetworkFileHandle,
            StdKind::StaleNetworkFileHandle,
        ),
        (IoErrorKind::InvalidInput, StdKind::InvalidInput),
        (IoErrorKind::InvalidData, StdKind::InvalidData),
        (IoErrorKind::TimedOut, StdKind::TimedOut),
        (IoErrorKind::WriteZero, StdKind::WriteZero),
        (IoErrorKind::StorageFull, StdKind::StorageFull),
        (IoErrorKind::NotSeekable, StdKind::NotSeekable),
        (IoErrorKind::QuotaExceeded, StdKind::QuotaExceeded),
        (IoErrorKind::FileTooLarge, StdKind::FileTooLarge),
        (IoErrorKind::ResourceBusy, StdKind::ResourceBusy),
        (IoErrorKind::ExecutableFileBusy, StdKind::ExecutableFileBusy),
        (IoErrorKind::Deadlock, StdKind::Deadlock),
        (IoErrorKind::CrossesDevices, StdKind::CrossesDevices),
        (IoErrorKind::TooManyLinks, StdKind::TooManyLinks),
        (IoErrorKind::InvalidFilename, StdKind::InvalidFilename),
        (
            IoErrorKind::ArgumentListTooLong,
            StdKind::ArgumentListTooLong,
        ),
        (IoErrorKind::Interrupted, StdKind::Interrupted),
        (IoErrorKind::Unsupported, StdKind::Unsupported),
        (IoErrorKind::UnexpectedEof, StdKind::UnexpectedEof),
        (IoErrorKind::OutOfMemory, StdKind::OutOfMemory),
    ];

    #[test]
    fn every_named_kind_survives_a_round_trip_through_std() {
        for (kind, std_kind) in NAMED_KINDS {
            assert_eq!(IoErrorKind::from(std_kind), kind, "{std_kind:?}");
            assert_eq!(StdKind::from(kind), std_kind, "{kind:?}");
        }
    }

    #[test]
    fn a_std_kind_with_no_arm_of_its_own_falls_back_to_other() {
        // `Other` is the one `std` kind this crate gives no arm: whatever `std` calls
        // something it does not, arrives here.
        assert_eq!(IoErrorKind::from(StdKind::Other), IoErrorKind::Other);
    }

    #[test]
    fn an_io_failure_keeps_its_kind_and_its_message() {
        let source = IoFailure::new(StdKind::PermissionDenied, "denied here");
        let carried = IoError::from(source);

        assert_eq!(carried.kind(), IoErrorKind::PermissionDenied);
        assert_eq!(carried.message(), "denied here");

        let back = IoFailure::from(carried);
        assert_eq!(back.kind(), StdKind::PermissionDenied);
        assert_eq!(back.to_string(), "denied here");
    }

    #[test]
    fn naming_an_io_failure_keeps_its_kind_and_its_message() {
        let carried = IoError::new(IoErrorKind::TimedOut, "too slow".to_owned());

        assert_eq!(carried.kind(), IoErrorKind::TimedOut);
        assert_eq!(carried.message(), "too slow");
        assert_eq!(carried.to_string(), "too slow");
        assert!(StdError::source(&carried).is_none());
    }

    #[test]
    fn a_bincode_failure_keeps_its_message() {
        let source = Box::new(bincode2::ErrorKind::Custom("bad tag".to_owned()));
        let carried = BincodeError::from(source);

        assert_eq!(carried.message(), "bad tag");
        assert_eq!(carried.into_bincode().to_string(), "bad tag");
    }

    #[test]
    fn naming_a_bincode_failure_keeps_its_message() {
        let carried = BincodeError::new("bad tag".to_owned());

        assert_eq!(carried.message(), "bad tag");
        assert_eq!(carried.to_string(), "bad tag");
        assert!(StdError::source(&carried).is_none());
        assert_eq!(carried.into_bincode().to_string(), "bad tag");
    }

    #[test]
    fn a_json_failure_keeps_what_it_said_and_where() {
        let source = serde_json::from_str::<u32>("").unwrap_err();
        let (line, column) = (source.line(), source.column());
        let carried = JsonError::from(source);

        assert!(!carried.message().is_empty());
        // The two are distinct on this input, so a swap between them could not pass.
        assert_ne!(line, column);
        assert_eq!(carried.line(), line);
        assert_eq!(carried.column(), column);
        assert_eq!(carried.clone().into_json().to_string(), carried.message());
    }

    #[test]
    fn naming_a_json_failure_keeps_what_it_said_and_where() {
        let carried = JsonError::new("expected a value".to_owned(), 3, 7);

        assert_eq!(carried.message(), "expected a value");
        assert_eq!(carried.line(), 3);
        assert_eq!(carried.column(), 7);
        assert_eq!(carried.to_string(), "expected a value");
        assert!(StdError::source(&carried).is_none());
        assert_eq!(carried.into_json().to_string(), "expected a value");
    }

    #[test]
    fn an_address_that_will_not_read_says_which() {
        let carried = AddrError::new("nowhere".to_owned());

        assert_eq!(carried.target(), "nowhere");
        assert!(carried.to_string().contains("nowhere"));
        assert!(StdError::source(&carried).is_none());
    }
}
