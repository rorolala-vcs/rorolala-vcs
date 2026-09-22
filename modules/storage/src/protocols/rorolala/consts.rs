//! What a store's layout and its choices are named by, in one place.

use crate::{Chunking, Codec};

/// The suffix a temporary file's name ends in.
pub(super) const TEMPORARY_SUFFIX: &str = ".tmp";

/// The directory loose objects are kept under, inside a store's root.
pub(super) const OBJECTS_DIR: &str = "obj";

/// The directory manifests are kept under, inside a store's root.
///
/// They are kept apart from the objects rather than beside them, because a manifest is what says
/// which chunks are spoken for: a cleanup can read the manifests and nothing else to tell a chunk
/// that is used from one nothing refers to.
pub(super) const MANIFEST_DIR: &str = "manifest";

/// The directory packs are kept under, inside a store's root.
pub(super) const PACKED_DIR: &str = "packed";

/// How many hex characters of a key name its first directory level.
pub(super) const SLICE_FIRST: usize = 2;

/// How many hex characters of a key name its second directory level.
pub(super) const SLICE_SECOND: usize = 2;

/// The cut a store writes content with unless it is told otherwise.
///
/// Content is written whole by default: cutting makes a store *smaller*, not *correct*, so it is
/// something a store is asked for — see [`with_cut`](super::RorolalaStorage::with_cut) — and the cuts
/// that pay for themselves without being asked for are the ones text and packed containers ask for.
pub(super) const DEFAULT_CUT: Chunking = Chunking::Whole;

/// The codec a store writes content with unless it is told otherwise.
///
/// Content is written as it came in by default, for the same reason: compressing makes a store
/// smaller, so it is something a store is asked for — see
/// [`with_codec`](super::RorolalaStorage::with_codec).
pub(super) const DEFAULT_CODEC: Codec = Codec::Raw;

/// How many bytes are read to tell what content is.
///
/// This is the same stretch `git` looks at to tell a file it can read as lines from one it cannot:
/// a zero byte anywhere in it is what says content is not text.
pub(super) const SNIFF_LEN: usize = 8000;

/// Text at or above this many bytes is cut as well as compressed.
///
/// Compressing text pays wherever it is; cutting it needs enough of it for the manifest that
/// remembers the chunks to be worth having.
pub(super) const TEXT_CUT_FROM: usize = 4 * 1024;

/// The cut text is written with once it is big enough to cut.
///
/// Lines are what an edit moves, so the chunks are line-sized rather than file-sized: a few lines
/// changed in a large file then costs the chunks those lines fall in.
pub(super) const TEXT_CUT: Chunking = Chunking::Text {
    min: 1024,
    average: 4096,
    max: 16384,
};
