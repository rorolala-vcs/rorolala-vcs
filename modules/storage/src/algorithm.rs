use crate::{Chunking, Codec};

/// How a file is to be written: what it is encoded with, and how it is cut.
///
/// The choice is the whole of the per-file "how", and it is deliberately a plain value: making
/// it is the backend's business — see
/// [`choose_algorithm`](crate::StorageBackend::choose_algorithm) — and everything after it is
/// deterministic, so the same content written with the same choice always lands as the same
/// bytes.
///
/// None of it is part of the key, which is the hash of the content and nothing else, so a file
/// re-written with a different choice keeps the name it had.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AlgorithmChoice {
    /// The encoding the payload is written in.
    codec: Codec,
    /// How the content is cut into chunks before it is written.
    chunking: Chunking,
}

impl AlgorithmChoice {
    /// A choice of `codec` and `chunking`.
    #[must_use]
    pub const fn new(codec: Codec, chunking: Chunking) -> Self {
        Self { codec, chunking }
    }

    /// The encoding the payload is written in.
    #[must_use]
    pub const fn codec(&self) -> Codec {
        self.codec
    }

    /// How the content is cut into chunks before it is written.
    #[must_use]
    pub const fn chunking(&self) -> Chunking {
        self.chunking
    }
}
