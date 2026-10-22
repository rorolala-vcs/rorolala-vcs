//! What a cut costs, worked out without a store.
//!
//! The model here reproduces what `write_content` does, entry for entry: one range is written as one
//! object, more than one is written as an object each plus a manifest, and an entry already held is
//! not written again. What comes out of that is exactly the bytes the store's `obj/` and `manifest/`
//! directories would hold, frame headers included — which is what keeps this model honest, and what
//! `store.rs` checks against a real store rather than asserts.
//!
//! The alternative — running a store for every measurement — would be slower than the cut by an order
//! of magnitude, and the model lets the same walk answer dedup, the cost of each version and the
//! manifest overhead in one pass.

// Byte counts are exact integers and the numbers derived from them are ratios of exact integers; an
// `f64` is the right shape for a score and the wrong shape for the bytes themselves.
#![allow(clippy::cast_precision_loss)]

use std::collections::HashSet;
use std::time::Instant;

use rorolala_storage::{Chunker as _, Chunking, FRAME_HEADER_LEN};

use crate::corpus::Pattern;
use crate::peak;

/// The bytes a manifest spends before its first record: its chunk count.
const MANIFEST_HEAD: u64 = 8;

/// The bytes each chunk costs the manifest: its digest and its length.
const MANIFEST_RECORD: u64 = 40;

/// How many times a cut is timed, keeping the fastest.
///
/// One run is not a measurement of a cut this cheap: the first one pays for touching pages the read
/// has just filled, and a cut of a positional kind does no memory traffic at all, so its time is the
/// allocator's rather than the content's. Three runs is what it takes for the number to stop moving.
const TIMINGS: usize = 3;

/// What one chunker costs one pattern.
#[derive(Debug, Default, Clone)]
pub struct Numbers {
    /// The bytes the content of every version is.
    pub content: u64,
    /// How many versions the pattern holds.
    pub versions: u64,
    /// How many chunks the content was cut into, over the whole pattern.
    pub chunks: u64,
    /// The bytes of the objects the store did not already hold.
    pub unique: u64,
    /// The bytes the manifests take.
    pub manifest: u64,
    /// The bytes the store would hold: frames, objects and manifests together.
    pub stored: u64,
    /// The largest share of a version the store did not already hold.
    pub worst_marginal: f64,
    /// The largest a split's own working set was, as a multiple of the version it cut.
    pub worst_peak: f64,
    /// The largest a split's working set was, in bytes.
    pub peak: u64,
    /// How long the cuts took, in nanoseconds.
    pub split_nanos: u128,
}

/// Whether a cut behaved.
#[derive(Debug, Clone, Copy)]
pub struct Gates {
    /// The ranges are in order, do not overlap, and cover the content exactly.
    pub covers: bool,
    /// The same content cuts the same way twice.
    pub stable: bool,
    /// The ranges put back together are the content.
    pub round_trip: bool,
}

/// What one chunker did to one pattern.
#[derive(Debug, Clone)]
pub struct Measured {
    /// What it cost.
    pub numbers: Numbers,
    /// Whether it behaved.
    pub gates: Gates,
}

impl Gates {
    /// Whether every gate passed.
    pub const fn ok(self) -> bool {
        self.covers && self.stable && self.round_trip
    }
}

impl Numbers {
    /// The share of the content the store did not already have, as a fraction of the content: 0 is
    /// everything deduplicated, 1 is nothing.
    pub fn undeduped(&self) -> f64 {
        if self.content == 0 {
            return 0.0;
        }

        self.unique as f64 / self.content as f64
    }

    /// The share of the content the manifests take.
    pub fn manifest_share(&self) -> f64 {
        if self.content == 0 {
            return 0.0;
        }

        self.manifest as f64 / self.content as f64
    }

    /// The share of the content the store holds.
    pub fn stored_share(&self) -> f64 {
        if self.content == 0 {
            return 0.0;
        }

        self.stored as f64 / self.content as f64
    }

    /// The content in bytes, as a rate of the cuts' own time.
    ///
    /// A cut fast enough for the clock to read as zero is answered with infinity rather than with
    /// zero: what happened is that the timer could not resolve it, and a rate of nothing would fail
    /// the floor and call the fastest cut in the table slow.
    pub fn speed(&self) -> f64 {
        if self.content == 0 {
            return 0.0;
        }

        if self.split_nanos == 0 {
            return f64::INFINITY;
        }

        let seconds = self.split_nanos as f64 / 1_000_000_000.0;

        self.content as f64 / seconds / 1_000_000.0
    }

    /// Folds another pattern's numbers into this one.
    ///
    /// Summed where they are sums, and taken at the worst where they are worsts: a policy is not
    /// forgiven its worst pattern by a good run somewhere else.
    pub fn merge(&mut self, other: &Self) {
        self.content = self.content.saturating_add(other.content);
        self.versions = self.versions.saturating_add(other.versions);
        self.chunks = self.chunks.saturating_add(other.chunks);
        self.unique = self.unique.saturating_add(other.unique);
        self.manifest = self.manifest.saturating_add(other.manifest);
        self.stored = self.stored.saturating_add(other.stored);
        self.worst_marginal = self.worst_marginal.max(other.worst_marginal);
        self.worst_peak = self.worst_peak.max(other.worst_peak);
        self.peak = self.peak.max(other.peak);
        self.split_nanos = self.split_nanos.saturating_add(other.split_nanos);
    }
}

impl Measured {
    /// Folds another pattern in, keeping the gates that failed.
    pub fn merge(&mut self, other: &Self) {
        self.numbers.merge(&other.numbers);
        self.gates.covers &= other.gates.covers;
        self.gates.stable &= other.gates.stable;
        self.gates.round_trip &= other.gates.round_trip;
    }
}

/// Cuts every version of `pattern` with `chunking`, and says what it cost.
///
/// # Errors
///
/// Returns whatever reading a version fails with.
pub fn measure(
    pattern: &Pattern,
    chunking: &Chunking,
) -> Result<Measured, Box<dyn std::error::Error>> {
    let mut objects: HashSet<[u8; 32]> = HashSet::new();
    let mut manifests: HashSet<[u8; 32]> = HashSet::new();
    let mut measured = Measured {
        numbers: Numbers::default(),
        gates: Gates {
            covers: true,
            stable: true,
            round_trip: true,
        },
    };

    for lane in &pattern.lanes {
        for (at, path) in lane.versions.iter().enumerate() {
            let content = std::fs::read(path)?;
            let len = u64::try_from(content.len()).map_err(|_| "a version longer than a `u64`")?;

            // A pattern's first version has nothing before it to be measured against, so its cost is
            // not a marginal one: it is all of it, which says nothing about whether an edit is cheap.
            let cold = measured.numbers.versions == 0;

            peak::reset();
            let started = Instant::now();
            let ranges = chunking.split(&content);
            let mut fastest = started.elapsed();

            // Read before the repeats, which allocate again: what the working set was is the first
            // cut's answer, and a later one would only be counting the same thing twice.
            let peak = u64::try_from(peak::peak()).unwrap_or(u64::MAX);
            let ratio = allocation(len);

            // The repeats answer two things at once: a rate that is not the first run's, and the
            // stability gate — the same content has to cut the same way, or one file has two
            // manifests and the store holds the same content twice under one key.
            for round in 1..TIMINGS {
                let started = Instant::now();
                let again = chunking.split(&content);
                fastest = fastest.min(started.elapsed());

                if at == 0 && round == 1 {
                    measured.gates.stable &= again == ranges;
                }
            }

            let numbers = &mut measured.numbers;
            numbers.peak = numbers.peak.max(peak);
            numbers.worst_peak = numbers.worst_peak.max(ratio);
            numbers.content = numbers.content.saturating_add(len);
            numbers.versions = numbers.versions.saturating_add(1);
            numbers.split_nanos = numbers.split_nanos.saturating_add(fastest.as_nanos());

            measured.gates.covers &= covers(&ranges, content.len());
            measured.gates.round_trip &= assembled(&content, &ranges) == content;

            let chunks = u64::try_from(ranges.len()).unwrap_or(u64::MAX);
            numbers.chunks = numbers.chunks.saturating_add(chunks);

            // One range is not a cutting: `write_content` lays the content down as a single object,
            // and the model has to say the same or the bytes it predicts are not the bytes there are.
            // What is new is counted either way, which is what the marginal cost is made of.
            let mut fresh = 0_u64;

            if ranges.len() < 2 {
                if objects.insert(digest(&content)) {
                    numbers.unique = numbers.unique.saturating_add(len);
                    numbers.stored = numbers.stored.saturating_add(frame() + len);
                    fresh = len;
                }
            } else {
                for range in &ranges {
                    let chunk: &[u8] = content.get(range.clone()).map_or(&[], |chunk| chunk);
                    if objects.insert(digest(chunk)) {
                        let size = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
                        numbers.unique = numbers.unique.saturating_add(size);
                        numbers.stored = numbers.stored.saturating_add(frame() + size);
                        fresh = fresh.saturating_add(size);
                    }
                }

                // A manifest is keyed by the content it puts together, so the same content written
                // twice has one manifest, exactly as the store's own skip does.
                let manifest = MANIFEST_HEAD.saturating_add(MANIFEST_RECORD.saturating_mul(chunks));
                if manifests.insert(digest(&content)) {
                    numbers.manifest = numbers.manifest.saturating_add(manifest);
                    numbers.stored = numbers.stored.saturating_add(frame() + manifest);
                }
            }

            if !cold && len > 0 {
                let marginal = fresh as f64 / len as f64;
                numbers.worst_marginal = numbers.worst_marginal.max(marginal);
            }
        }
    }

    Ok(measured)
}

/// The bytes a frame header takes in front of one entry.
const fn frame() -> u64 {
    FRAME_HEADER_LEN as u64
}

/// The key a piece of content is stored under.
fn digest(content: &[u8]) -> [u8; 32] {
    *blake3::hash(content).as_bytes()
}

/// The multiple of the version's own length a split's working set came to, over what it was given.
///
/// The content is already held by the caller, so what this counts is everything the heap had live at
/// its highest — the content included. A cut that copies its input once reads as 2, and that is the
/// number worth having: it is what a large file costs to cut.
fn allocation(len: u64) -> f64 {
    if len == 0 {
        return 0.0;
    }

    peak::peak() as f64 / len as f64
}

/// Whether the ranges are in order, do not overlap, and cover `len` bytes exactly.
fn covers(ranges: &[std::ops::Range<usize>], len: usize) -> bool {
    let mut at = 0;

    for range in ranges {
        if range.start != at || range.end < range.start {
            return false;
        }
        at = range.end;
    }

    at == len
}

/// The content the ranges name, put back together.
fn assembled(content: &[u8], ranges: &[std::ops::Range<usize>]) -> Vec<u8> {
    let mut put_back = Vec::with_capacity(content.len());

    for range in ranges {
        if let Some(piece) = content.get(range.clone()) {
            put_back.extend_from_slice(piece);
        }
    }

    put_back
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rorolala_storage::Chunking;

    use super::{covers, measure};
    use crate::corpus::Corpus;
    use crate::generate::{Scale, synthetic};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        crate::scratch::at(label)
    }

    /// One pattern of the corpus written under `root`.
    fn one_pattern(root: &std::path::Path, pattern: &str) -> crate::corpus::Pattern {
        synthetic(root, "synthetic", Scale::Test).unwrap();

        let mut corpus = Corpus::read(root).unwrap();
        let mut policy = corpus.policies.remove(0);
        let at = policy
            .patterns
            .iter()
            .position(|found| found.name == pattern)
            .unwrap();

        policy.patterns.remove(at)
    }

    #[test]
    fn a_cut_that_covers_nothing_is_not_a_cut() {
        // The three ways ranges are wrong: a gap, an overlap, and stopping short.
        assert!(!covers(&[0..2, 3..4], 4));
        assert!(!covers(&[0..3, 2..4], 4));
        assert!(!covers(std::slice::from_ref(&(0..2)), 4));
        assert!(covers(&[0..2, 2..4], 4));
        assert!(covers(&[], 0));
    }

    #[test]
    fn the_model_of_a_whole_write_is_one_object() {
        let root = scratch("whole");
        let pattern = one_pattern(&root, "big-file");
        let measured = measure(&pattern, &Chunking::Whole).unwrap();

        // One version, one range, one object: nothing is deduplicated and nothing is a manifest.
        assert_eq!(measured.numbers.versions, 1);
        assert_eq!(measured.numbers.chunks, 1);
        assert_eq!(measured.numbers.manifest, 0);
        assert_eq!(measured.numbers.unique, measured.numbers.content);
        assert!(measured.gates.ok());
    }

    #[test]
    fn the_same_bytes_written_twice_are_one_object() {
        let root = scratch("rewrite");
        let pattern = one_pattern(&root, "full-rewrite");
        let measured = measure(&pattern, &Chunking::Whole).unwrap();

        // The second version is the first one's bytes, so the store holds one object for both and
        // the second version costs nothing — which is what a content-addressed store is for.
        assert_eq!(measured.numbers.versions, 2);
        assert_eq!(measured.numbers.unique, measured.numbers.content / 2);
        assert!(measured.numbers.worst_marginal.abs() < f64::EPSILON);
    }

    #[test]
    fn a_cut_policy_pays_a_manifest_and_holds_less() {
        let root = scratch("cdc");
        let pattern = one_pattern(&root, "in-place-edit");
        let whole = measure(&pattern, &Chunking::Whole).unwrap();
        let cdc = measure(
            &pattern,
            &Chunking::Cdc {
                min: 256,
                average: 1024,
                max: 8192,
            },
        )
        .unwrap();

        assert!(cdc.numbers.manifest > 0);
        assert!(cdc.numbers.chunks > 1);
        // Three versions of one document with an edit in each: cutting on content has to hold less
        // than keeping the whole document three times.
        assert!(
            cdc.numbers.unique < whole.numbers.unique,
            "{} !< {}",
            cdc.numbers.unique,
            whole.numbers.unique
        );
        assert!(cdc.gates.ok());
    }

    #[test]
    fn a_whole_write_shares_nothing_between_two_paths() {
        let root = scratch("shared");
        let pattern = one_pattern(&root, "shared-asset");

        // Three versions, three whole objects: a cut by position is no help across paths either.
        let whole = measure(&pattern, &Chunking::Whole).unwrap();
        assert_eq!(whole.numbers.versions, 3);
        assert_eq!(whole.numbers.unique, whole.numbers.content);

        // The same body in two paths, each edited somewhere else: a cut on content is what finds the
        // parts the two still have in common, which is the only reason the pattern has two lanes.
        let cdc = measure(
            &pattern,
            &Chunking::Cdc {
                min: 256,
                average: 1024,
                max: 8192,
            },
        )
        .unwrap();
        assert!(
            cdc.numbers.unique < cdc.numbers.content,
            "{} !< {}",
            cdc.numbers.unique,
            cdc.numbers.content
        );
    }
}
