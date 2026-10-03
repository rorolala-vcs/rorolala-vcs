//! Naming a hash by the head of it.
//!
//! A digest is long enough that a person reads and writes only its head, so a command that takes a
//! hash reads either the whole thing or enough of the front that only one object the run can reach
//! starts with it. Which objects those are is the command's own question — a `vcs-index` write
//! names a version where a `storage` read names a key — so the candidates are handed in.
//!
//! A whole hash is read on its own and never looked up: a command that takes a hash it need not
//! have — `vcs-index write-variant` records one it never checked — must go on taking it. Only a
//! head is resolved, and only against what the run can list.

use std::str::FromStr as _;

use librorolala::storage::{Key, Resolved, ShortHash};
use rust_i18n::t;

/// Why a word named no one hash.
pub enum HashMiss {
    /// It reads as neither a hash nor the head of one.
    Malformed,
    /// It reads as a head, and nothing the run can reach starts with it.
    Unknown,
    /// It reads as a head, and more than one thing the run can reach starts with it.
    Ambiguous(Vec<String>),
}

impl HashMiss {
    /// The reason, drawn the area's own way for a malformed hash and the shared way otherwise.
    ///
    /// `malformed` is the area's own words: every command has said what a hash looks like, and a
    /// word that is not one still gets them.
    pub fn reason(&self, hash: &str, malformed: impl FnOnce() -> String) -> String {
        match self {
            Self::Malformed => malformed(),
            Self::Unknown => t!("common.err_hash_unknown", hash = hash)
                .trim()
                .to_string(),
            Self::Ambiguous(candidates) => t!(
                "common.err_hash_ambiguous",
                hash = hash,
                candidates = candidates.join(", ")
            )
            .trim()
            .to_string(),
        }
    }

    /// The help line, which says what to do about it.
    ///
    /// `malformed` is the area's own help for a word that is not a hash; a head that names nothing
    /// or several things is the same advice wherever it was typed, so it is said once.
    pub fn help(&self, malformed: &str) -> String {
        match self {
            Self::Malformed => malformed.to_owned(),
            Self::Unknown | Self::Ambiguous(_) => t!("common.err_hash_help").trim().to_owned(),
        }
    }
}

/// Whether `text` is shaped like a hash or the head of one, rather than like a name or a path.
///
/// A command that tells a hash from a Vault's name by shape asks this: a head is as much a hash as
/// a whole one, so a name of four hex digits is read as a hash — the price of naming a hash by its
/// head without a marker in front of it.
#[must_use]
pub fn looks_like_hash(text: &str) -> bool {
    Key::from_str(text).is_ok() || ShortHash::new(text).is_ok()
}

/// Reads `text` as a whole hash, or as the head of one among `candidates`.
///
/// The candidates are asked for only when `text` is a head: a whole hash is read on its own, so a
/// command that takes one pays no listing.
///
/// # Errors
///
/// [`HashMiss::Malformed`] when it reads as neither, [`HashMiss::Unknown`] when it reads as a head
/// nothing starts with, and [`HashMiss::Ambiguous`] when several things do.
pub fn resolve(text: &str, candidates: impl FnOnce() -> Vec<Key>) -> Result<Key, HashMiss> {
    if let Ok(key) = Key::from_str(text) {
        return Ok(key);
    }

    resolve_among(text, &candidates())
}

/// Reads `text` as `resolve` does, with the candidates already in hand.
///
/// It is [`resolve`] for a caller that has the listing already — a command resolving several words
/// against one listing lists once. A whole hash is still read on its own: resolving it against a
/// listing would refuse a hash the run does not hold, which a sync of one is allowed to name.
///
/// # Errors
///
/// As [`resolve`].
pub fn resolve_among(text: &str, candidates: &[Key]) -> Result<Key, HashMiss> {
    if let Ok(key) = Key::from_str(text) {
        return Ok(key);
    }

    let Ok(head) = ShortHash::new(text) else {
        return Err(HashMiss::Malformed);
    };

    match head.resolve(candidates.iter()) {
        Resolved::ExactlyOne(key) => Ok(key),
        Resolved::NoMatch => Err(HashMiss::Unknown),
        Resolved::Ambiguous(keys) => Err(HashMiss::Ambiguous(keys.iter().map(Key::hex).collect())),
    }
}
