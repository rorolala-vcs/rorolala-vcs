//! A synthetic corpus, for exercising the harness before there is a strategy to measure.
//!
//! This is not a format. It is deterministic noise with records in it, cut and rewritten the five
//! ways this harness judges a cut: a large first version, an equal-length rewrite in the middle, a
//! body kept while the header moves, the same bytes written again, and one body shared by two paths.
//!
//! It exists so that the yardstick can be shown to work — and, in the test beside [`measure`], so
//! that the model of the store can be shown to agree with the store byte for byte. It is **not** a
//! corpus to judge a format policy by: a policy's corpus is captured from the tool that writes that
//! format, because a synthetic Maya file would only measure this generator's opinion of Maya.
//!
//! [`measure`]: crate::measure

use std::fs;
use std::path::Path;

/// How large the chains are.
///
/// The test scale is a sixteenth of the full one, so a unit test that writes a corpus and runs a
/// store over it stays a test rather than a wait. Both scales produce the same *shape*; only the
/// sizes differ, which is what a test of the model needs and what a report of a strategy does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    /// A corpus worth reading numbers off.
    Full,
    /// A corpus worth asserting against.
    // A corpus this small is made by the tests and by nothing else: the program's own runs want a
    // chain long enough for a chunk-size distribution to mean anything.
    #[cfg_attr(not(test), allow(dead_code))]
    Test,
}

impl Scale {
    /// The size of one "unit" of content, which every chain is a multiple of.
    const fn unit(self) -> usize {
        match self {
            Self::Full => 1024 * 1024,
            Self::Test => 64 * 1024,
        }
    }
}

/// The container the synthetic versions are written with.
///
/// One that no format policy claims: a synthetic chain must never be mistaken for a real one, and an
/// extension is how a policy would otherwise claim it.
const CONTAINER: &str = "bin";

/// How many bytes one record of a body is, before its header.
const RECORD: usize = 4096;

/// Writes a synthetic corpus for `name` under `root`.
///
/// # Errors
///
/// Returns whatever making the directories or writing the versions fails with.
pub fn synthetic(root: &Path, name: &str, scale: Scale) -> Result<(), Box<dyn std::error::Error>> {
    let unit = scale.unit();
    let policy = root.join(name);

    // A large first version, with no history: the cost of one version and the size of the chunks it
    // comes out as.
    write(&policy.join("big-file"), &[body("big", unit * 2)])?;

    // An equal-length rewrite in the middle: what a save of a large document looks like, and the
    // case a content-defined cut exists for.
    //
    // The regions are deliberately not aligned to any power of two. An edit that began and ended on
    // a 64 KiB boundary would cost a positional cut exactly one chunk, which is a property of the
    // arithmetic rather than of the strategy — the corpus must not hand one kind of cut a win.
    let original = body("edit", unit);
    let mut first = original.clone();
    let mut second = original.clone();
    rewritten(&mut first, unit / 2 + 1234, unit / 16 - 777, "edit-a");
    rewritten(&mut second, unit / 4 + 4321, unit / 16 - 777, "edit-b");
    write(&policy.join("in-place-edit"), &[original, first, second])?;

    // The body kept and the header moved: the case a container-aware policy exists to win. The
    // header is not a round number either, for the same reason.
    let header = unit / 64 + 333;
    let bodied = body("bodied", unit);
    let mut original = body("header", header);
    original.extend_from_slice(&bodied);
    let mut renewed = body("header-2", header);
    renewed.extend_from_slice(&bodied);
    write(&policy.join("metadata-only"), &[original, renewed])?;

    // The same bytes written again: whether a cut hits when nothing about the content changed.
    let kept = body("kept", unit);
    write(&policy.join("full-rewrite"), &[kept.clone(), kept])?;

    // One body shared by two paths, each edited somewhere else: deduplication between files, which
    // no single chain shows.
    let shared = body("shared", unit);
    let mut left = shared.clone();
    let mut right = shared.clone();
    rewritten(&mut left, unit / 8 + 99, unit / 16 - 777, "shared-left");
    rewritten(
        &mut right,
        unit * 3 / 4 + 313,
        unit / 16 - 777,
        "shared-right",
    );
    let asset = policy.join("shared-asset");
    write(&asset.join("a"), &[shared, left])?;
    write(&asset.join("b"), &[right])?;

    Ok(())
}

/// Writes `versions` into `directory` as `000.<container>`, `001.<container>`, …
fn write(directory: &Path, versions: &[Vec<u8>]) -> std::io::Result<()> {
    fs::create_dir_all(directory)?;

    for (number, content) in versions.iter().enumerate() {
        fs::write(directory.join(format!("{number:03}.{CONTAINER}")), content)?;
    }

    Ok(())
}

/// A body of `size` bytes made of records.
///
/// Records rather than pure noise: real files have structure, and a cut that ignores structure should
/// not be measured against content that has none. Each record is a tag, a length and its bytes, and
/// the bytes come from a stream seeded by `label` so two bodies differ throughout.
fn body(label: &str, size: usize) -> Vec<u8> {
    let mut noise = Noise::new(seed(label));
    let mut bytes = Vec::with_capacity(size);
    let mut tag = 0_u32;

    // 4096, which is a `u32` and is why the conversion cannot fail.
    let width = u32::try_from(RECORD).unwrap_or(u32::MAX);

    while bytes.len() < size {
        bytes.extend_from_slice(&tag.to_le_bytes());
        bytes.extend_from_slice(&width.to_le_bytes());

        let take = (size - bytes.len()).min(RECORD);
        let mut record = vec![0_u8; take];
        noise.fill(&mut record);
        bytes.extend_from_slice(&record);

        tag = tag.wrapping_add(1);
    }

    bytes.truncate(size);

    bytes
}

/// Replaces `size` bytes of `content` at `at` with the bytes of another stream.
fn rewritten(content: &mut [u8], at: usize, size: usize, label: &str) {
    let mut noise = Noise::new(seed(label));
    let start = at.min(content.len());
    let end = start.saturating_add(size).min(content.len());

    if let Some(region) = content.get_mut(start..end) {
        noise.fill(region);
    }
}

/// The seed a label stands for.
///
/// FNV-1a, because it is four lines and it is stable: a corpus that changed when the compiler did
/// would change every score with it.
fn seed(label: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in label.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    hash
}

/// A deterministic stream of bytes.
struct Noise {
    /// The whole of the stream's state: `splitmix64`'s counter.
    state: u64,
}

impl Noise {
    /// A stream seeded by `seed`.
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next word.
    const fn word(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut mixed = self.state;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);

        mixed ^ (mixed >> 31)
    }

    /// Fills `out` with the stream.
    fn fill(&mut self, out: &mut [u8]) {
        for part in out.chunks_mut(size_of::<u64>()) {
            let word = self.word().to_le_bytes();
            part.copy_from_slice(&word[..part.len()]);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{Scale, synthetic};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch() -> PathBuf {
        crate::scratch::at("generate")
    }

    /// Every version of every chain, as whole values, in path order.
    fn everything(root: &std::path::Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut found = Vec::new();
        let mut stack = vec![root.to_path_buf()];

        while let Some(at) = stack.pop() {
            for entry in fs::read_dir(&at).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    found.push((path.clone(), fs::read(&path).unwrap()));
                }
            }
        }

        found.sort();

        found
    }

    #[test]
    fn a_corpus_written_twice_is_the_same_bytes() {
        let first = scratch();
        synthetic(&first, "synthetic", Scale::Test).unwrap();

        let once = everything(&first);
        synthetic(&first, "synthetic", Scale::Test).unwrap();
        let twice = everything(&first);

        // A corpus that moved between runs would move every score with it.
        assert_eq!(once, twice);
        assert_ne!(once.len(), 0);
    }

    #[test]
    fn every_pattern_the_harness_judges_is_written() {
        let root = scratch();
        synthetic(&root, "synthetic", Scale::Test).unwrap();

        let mut patterns: Vec<String> = fs::read_dir(root.join("synthetic"))
            .unwrap()
            .map(|entry| {
                entry
                    .unwrap()
                    .path()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        patterns.sort();

        assert_eq!(
            patterns,
            [
                "big-file",
                "full-rewrite",
                "in-place-edit",
                "metadata-only",
                "shared-asset"
            ]
        );
    }

    #[test]
    fn a_rewrite_keeps_the_content_the_same_length() {
        let root = scratch();
        synthetic(&root, "synthetic", Scale::Test).unwrap();

        let chain: Vec<Vec<u8>> = (0..3)
            .map(|number| {
                fs::read(root.join(format!("synthetic/in-place-edit/{number:03}.bin"))).unwrap()
            })
            .collect();

        // An edit that changed the length would be a different pattern: this one is about where the
        // bytes changed, not how many there are.
        assert_eq!(chain[0].len(), chain[1].len());
        assert_eq!(chain[1].len(), chain[2].len());
        assert_ne!(chain[0], chain[1]);
    }
}
