//! Naming one level of a chain by what a person writes.

use std::fmt;
use std::str::FromStr as _;

use rorolala_storage::{Blake3Hash, Key, Resolved, ShortHash};
use rorolala_vcs::VCSWrite as _;

use crate::Level;

/// How many hex digits a hash is written with when a sheet names it.
const SHORT: usize = 12;

/// The longest a version's head is drawn as when it stands beside its number.
const HEAD: usize = 7;

/// The hash of a digest, as a sheet names it.
#[must_use]
pub fn short(digest: &Blake3Hash) -> String {
    let hex = Key::new(*digest).hex();

    hex[..hex.len().min(SHORT)].to_owned()
}

/// A name of a version for a person: its number when it is known, and the head of its hash.
#[must_use]
pub fn named(version: &str, number: usize) -> String {
    let head = &version[..version.len().min(HEAD)];

    format!("V{number} ({head})")
}

/// Why a hash named no one object.
#[derive(Debug)]
pub enum HeadMiss {
    /// It is neither a whole key nor the head of one.
    Malformed,
    /// Nothing the run can reach starts with it.
    Unknown,
    /// More than one thing does.
    Ambiguous,
}

/// Reads `text` as a whole key, or as the head of one among `candidates`.
///
/// # Errors
///
/// Returns [`HeadMiss`] when `text` is neither a whole key nor the head of one, when nothing among
/// `candidates` starts with it, or when more than one does.
pub fn resolve_head(text: &str, candidates: &[Key]) -> Result<Key, HeadMiss> {
    if let Ok(key) = Key::from_str(text) {
        return Ok(key);
    }

    let Ok(head) = ShortHash::new(text) else {
        return Err(HeadMiss::Malformed);
    };

    match head.resolve(candidates) {
        Resolved::ExactlyOne(key) => Ok(key),
        Resolved::NoMatch => Err(HeadMiss::Unknown),
        Resolved::Ambiguous(_) => Err(HeadMiss::Ambiguous),
    }
}

/// Whether `text` names `digest`, whether it is the whole key or the head of one.
#[must_use]
pub fn names(text: &str, digest: &Blake3Hash) -> bool {
    if let Ok(key) = Key::from_str(text) {
        return key.digest() == digest;
    }

    ShortHash::new(text).is_ok_and(|head| head.matches(&Key::new(*digest)))
}

/// Why a run named no version it can adjust.
#[derive(Debug)]
pub enum TargetMiss {
    /// Nothing on the chain, and nothing the index holds, matches what was written.
    NoMatch {
        /// What was written.
        target: String,
    },
    /// More than one version of the chain matches.
    Ambiguous {
        /// What was written.
        target: String,
        /// The versions it matches, as numbers.
        candidates: Vec<usize>,
    },
    /// The hash names an object that is not on this chain.
    Elsewhere {
        /// What was written.
        target: String,
    },
    /// A step count reaches past the first version.
    OutOfChain {
        /// What was written.
        target: String,
    },
    /// The version is one the plan already removes.
    Removed {
        /// The version, as a name for a person.
        version: String,
    },
}

impl fmt::Display for TargetMiss {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoMatch { target } => write!(formatter, "nothing matches {target}"),
            Self::Ambiguous { target, .. } => write!(formatter, "{target} matches more than one"),
            Self::Elsewhere { target } => write!(formatter, "{target} belongs to another chain"),
            Self::OutOfChain { target } => {
                write!(formatter, "{target} reaches past the first version")
            }
            Self::Removed { version } => write!(formatter, "{version} is being removed"),
        }
    }
}

impl std::error::Error for TargetMiss {}

/// The number of the version `text` names among `levels`' own.
///
/// # Errors
///
/// Returns [`HeadMiss`] when `text` names no version, or more than one.
pub fn number_of(levels: &[Level], text: &str) -> Result<usize, HeadMiss> {
    let keys: Vec<Key> = levels
        .iter()
        .map(|level| Key::new(*level.version.hash().digest()))
        .collect();
    let key = resolve_head(text, &keys)?;

    keys.iter()
        .position(|candidate| *candidate == key)
        .ok_or(HeadMiss::Unknown)
}

/// The versions whose names start with the head `text` writes.
#[must_use]
pub fn versions_naming(levels: &[Level], text: &str) -> Vec<usize> {
    let Ok(head) = ShortHash::new(text) else {
        return Vec::new();
    };

    levels
        .iter()
        .enumerate()
        .filter(|(_, level)| head.matches(&Key::new(*level.version.hash().digest())))
        .map(|(number, _)| number)
        .collect()
}

/// The version a run names, by its number on the chain.
///
/// `~N` is N versions back from `top`, a hash is a version or the variant that produced one, and
/// anything else is text matched against the creators and the messages, which names a version only
/// when it names one.
///
/// # Errors
///
/// Returns [`TargetMiss`] when the target names no version, or more than one.
pub fn resolve(levels: &[Level], top: usize, target: Option<&str>) -> Result<usize, TargetMiss> {
    let Some(text) = target else {
        return Ok(top);
    };

    if let Some(steps) = text.strip_prefix('~') {
        let steps: usize = steps.parse().map_err(|_| TargetMiss::NoMatch {
            target: text.to_owned(),
        })?;

        return top
            .checked_sub(steps)
            .ok_or_else(|| TargetMiss::OutOfChain {
                target: text.to_owned(),
            });
    }

    match number_of(levels, text) {
        Ok(number) => return Ok(number),
        Err(HeadMiss::Ambiguous) => {
            return Err(TargetMiss::Ambiguous {
                target: text.to_owned(),
                candidates: versions_naming(levels, text),
            });
        }
        Err(HeadMiss::Malformed | HeadMiss::Unknown) => {}
    }

    // The variant that produced a version names that version, which is what a run that read the
    // hash off a drawing means.
    let variants: Vec<Key> = levels
        .iter()
        .map(|level| Key::new(*level.variant.hash().digest()))
        .collect();
    if let Ok(variant) = resolve_head(text, &variants)
        && let Some(number) = variants.iter().position(|candidate| *candidate == variant)
    {
        return Ok(number);
    }

    // A word that is neither a hash nor the head of one is text, and so is a head that names
    // nothing here: a message is allowed to look like a hash.
    let needle = text.to_lowercase();
    let hits: Vec<usize> = levels
        .iter()
        .enumerate()
        .filter(|(_, level)| {
            level.creator.to_lowercase().contains(&needle)
                || level.message.to_lowercase().contains(&needle)
        })
        .map(|(number, _)| number)
        .collect();

    match hits.len() {
        0 => Err(if is_a_hash(text) {
            TargetMiss::Elsewhere {
                target: text.to_owned(),
            }
        } else {
            TargetMiss::NoMatch {
                target: text.to_owned(),
            }
        }),
        1 => Ok(hits[0]),
        _ => Err(TargetMiss::Ambiguous {
            target: text.to_owned(),
            candidates: hits,
        }),
    }
}

/// Whether `text` reads as a hash, whole or as a head, rather than as a word.
fn is_a_hash(text: &str) -> bool {
    Key::from_str(text).is_ok() || ShortHash::new(text).is_ok()
}

/// What a miss at naming a head is told as.
#[must_use]
pub fn miss_of(text: &str, miss: &HeadMiss) -> TargetMiss {
    match miss {
        HeadMiss::Malformed => TargetMiss::NoMatch {
            target: text.to_owned(),
        },
        HeadMiss::Unknown => TargetMiss::Elsewhere {
            target: text.to_owned(),
        },
        HeadMiss::Ambiguous => TargetMiss::Ambiguous {
            target: text.to_owned(),
            candidates: Vec::new(),
        },
    }
}
