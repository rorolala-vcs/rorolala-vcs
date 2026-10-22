//! Where the chains are, and what a chain is.
//!
//! A corpus is a directory of policies, a policy is a directory of patterns, and a pattern is either
//! one lane of versions or several. A lane is one path's history: `000.<ext>`, `001.<ext>`, … in the
//! order the numbers name. Nothing about a chain is written down anywhere but the file names, so what
//! an author adds is files and the harness reads them.
//!
//! Two lanes in one pattern is the case of a resource shared between two paths — the same body in two
//! files, each edited somewhere else. It is measured as one chain, because what the two have in common
//! is deduplication that only a cross-file measurement sees.

use std::fs;
use std::path::{Path, PathBuf};

/// Everything under one corpus directory.
#[derive(Debug)]
pub struct Corpus {
    /// One entry per policy directory, in name order.
    pub policies: Vec<Policy>,
}

/// One policy's chains: the directory named after the policy.
#[derive(Debug)]
pub struct Policy {
    /// The registered policy name this corpus belongs to.
    pub name: String,
    /// Where it sits, so a policy's own files — its description, its score — are found beside it.
    pub directory: PathBuf,
    /// One entry per pattern directory, in name order.
    pub patterns: Vec<Pattern>,
}

/// One pattern: the chain, or the chains, that exercise it.
#[derive(Debug)]
pub struct Pattern {
    /// The pattern's name, which is the name of its directory.
    pub name: String,
    /// The paths the pattern is made of, in name order. One, unless the pattern is a shared asset.
    pub lanes: Vec<Lane>,
}

/// One path's history.
#[derive(Debug)]
pub struct Lane {
    /// The name of the path, which is `main` when the pattern has no lane directories.
    pub name: String,
    /// The versions, in the order the numbers name.
    pub versions: Vec<PathBuf>,
}

impl Corpus {
    /// Reads the corpus under `root`.
    ///
    /// # Errors
    ///
    /// Returns a description of the first thing that is not a corpus: a pattern with no version, a
    /// version number that is not a number, numbers that do not start at zero and count by one, a
    /// lane whose versions are in more than one container, or a pattern that is neither versions nor
    /// lanes.
    pub fn read(root: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut policies = Vec::new();

        for policy_dir in sorted_dirs(root)? {
            let Some(name) = file_name(&policy_dir) else {
                continue;
            };

            let mut patterns = Vec::new();
            for pattern_dir in sorted_dirs(&policy_dir)? {
                let Some(pattern_name) = file_name(&pattern_dir) else {
                    continue;
                };

                patterns.push(read_pattern(&pattern_dir, &pattern_name)?);
            }

            policies.push(Policy {
                name,
                directory: policy_dir,
                patterns,
            });
        }

        Ok(Self { policies })
    }

    /// How many patterns the whole corpus holds.
    pub fn patterns(&self) -> usize {
        self.policies
            .iter()
            .map(|policy| policy.patterns.len())
            .sum()
    }
}

impl Pattern {
    /// Every version of every lane, in the order they are written.
    pub fn versions(&self) -> impl Iterator<Item = &PathBuf> {
        self.lanes.iter().flat_map(|lane| lane.versions.iter())
    }

    /// How many versions the pattern is made of.
    pub fn len(&self) -> usize {
        self.lanes.iter().map(|lane| lane.versions.len()).sum()
    }
}

/// Reads one pattern directory.
fn read_pattern(directory: &Path, name: &str) -> Result<Pattern, Box<dyn std::error::Error>> {
    let mut versions = Vec::new();
    let mut lanes = Vec::new();

    for entry in fs::read_dir(directory)? {
        let path = entry?.path();

        if path.is_dir() {
            let Some(lane_name) = file_name(&path) else {
                continue;
            };
            lanes.push(read_lane(&path, &lane_name)?);
        } else {
            versions.push(path);
        }
    }

    // A pattern is versions or it is lanes; one of each is a directory nobody meant.
    if !versions.is_empty() && !lanes.is_empty() {
        return Err(format!(
            "{}: holds versions and lane directories at once",
            directory.display()
        )
        .into());
    }

    if !versions.is_empty() {
        return Ok(Pattern {
            name: name.to_owned(),
            lanes: vec![Lane {
                name: "main".to_owned(),
                versions: ordered(versions, directory)?,
            }],
        });
    }

    lanes.sort_by(|left, right| left.name.cmp(&right.name));

    if lanes.is_empty() {
        return Err(format!("{}: names no version", directory.display()).into());
    }

    Ok(Pattern {
        name: name.to_owned(),
        lanes,
    })
}

/// Reads one lane's versions.
fn read_lane(directory: &Path, name: &str) -> Result<Lane, Box<dyn std::error::Error>> {
    let mut versions = Vec::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            return Err(
                format!("{}: a lane holds versions, not directories", path.display()).into(),
            );
        }
        versions.push(path);
    }

    Ok(Lane {
        name: name.to_owned(),
        versions: ordered(versions, directory)?,
    })
}

/// Puts a lane's versions in the order their numbers name, refusing anything that is not that order.
fn ordered(
    versions: Vec<PathBuf>,
    directory: &Path,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    if versions.is_empty() {
        return Err(format!("{}: names no version", directory.display()).into());
    }

    let mut numbered = Vec::with_capacity(versions.len());
    let mut extension: Option<String> = None;

    for path in versions {
        let Some(number) = path.file_stem().and_then(|stem| stem.to_str()) else {
            return Err(format!("{}: the name is not text", path.display()).into());
        };
        let Some(container) = path.extension().and_then(|extension| extension.to_str()) else {
            return Err(format!("{}: the name has no container", path.display()).into());
        };

        let number: u32 = number
            .parse()
            .map_err(|_| format!("{}: the version number is not a number", path.display()))?;

        match &extension {
            Some(kept) if kept != container => {
                return Err(format!(
                    "{}: a lane is one container, and this is `{container}` beside `{kept}`",
                    path.display()
                )
                .into());
            }
            Some(_) => {}
            None => extension = Some(container.to_owned()),
        }

        numbered.push((number, path));
    }

    numbered.sort_by_key(|(number, _)| *number);

    // A chain is every version from the first on, in order: a gap is a version somebody deleted, and
    // measuring around it would compare a history nobody has.
    for (expected, (number, path)) in numbered.iter().enumerate() {
        let expected = u32::try_from(expected).map_err(|_| "a lane is longer than a `u32`")?;
        if *number != expected {
            return Err(format!(
                "{}: versions start at 0 and count by one, and `{expected}` is not here",
                path.display()
            )
            .into());
        }
    }

    Ok(numbered.into_iter().map(|(_, path)| path).collect())
}

/// The directories under `directory`, in name order.
fn sorted_dirs(directory: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut dirs = Vec::new();

    // A corpus directory that is not there yet is nothing to measure rather than a failure: the first
    // thing an author does is make one, and the harness is run before it exists often enough.
    if !directory.is_dir() {
        return Ok(dirs);
    }

    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }

    dirs.sort();

    Ok(dirs)
}

/// The last component of `path`, when it is text.
fn file_name(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::Corpus;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        crate::scratch::at(label)
    }

    /// Writes a chain of `count` versions under `dir`, each named by its number and container.
    fn chain(dir: &Path, first: u32, count: u32, container: &str) {
        fs::create_dir_all(dir).unwrap();
        for number in first..first + count {
            fs::write(dir.join(format!("{number:03}.{container}")), b"x").unwrap();
        }
    }

    #[test]
    fn a_pattern_of_versions_reads_in_its_own_order() {
        let root = scratch("order");
        let pattern = root.join("alpha").join("big-file");
        // Written out of order and with a gap-free run, so what is read is the numbers' order.
        fs::create_dir_all(&pattern).unwrap();
        for number in [2, 0, 1] {
            fs::write(pattern.join(format!("{number:03}.mb")), b"x").unwrap();
        }

        let corpus = Corpus::read(&root).unwrap();
        let lane = &corpus.policies[0].patterns[0].lanes[0];

        assert_eq!(lane.name, "main");
        assert_eq!(
            lane.versions
                .iter()
                .filter_map(|path| path.file_name()?.to_str())
                .collect::<Vec<_>>(),
            ["000.mb", "001.mb", "002.mb"]
        );
    }

    #[test]
    fn a_gap_in_the_versions_is_refused() {
        let root = scratch("gap");
        chain(&root.join("alpha").join("big-file"), 0, 1, "mb");
        fs::write(root.join("alpha/big-file/002.mb"), b"x").unwrap();

        let error = Corpus::read(&root).unwrap_err().to_string();

        assert!(error.contains("start at 0"), "{error}");
    }

    #[test]
    fn a_lane_of_two_containers_is_refused() {
        let root = scratch("containers");
        let pattern = root.join("alpha").join("big-file");
        fs::create_dir_all(&pattern).unwrap();
        fs::write(pattern.join("000.mb"), b"x").unwrap();
        fs::write(pattern.join("001.ma"), b"x").unwrap();

        let error = Corpus::read(&root).unwrap_err().to_string();

        assert!(error.contains("one container"), "{error}");
    }

    #[test]
    fn lanes_are_read_in_name_order() {
        let root = scratch("lanes");
        chain(&root.join("alpha/shared-asset/b"), 0, 1, "psd");
        chain(&root.join("alpha/shared-asset/a"), 0, 1, "psd");

        let corpus = Corpus::read(&root).unwrap();
        let pattern = &corpus.policies[0].patterns[0];

        assert_eq!(
            pattern
                .lanes
                .iter()
                .map(|lane| lane.name.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
    }

    #[test]
    fn a_corpus_that_is_not_there_is_nothing_to_measure() {
        let root = scratch("empty");

        assert!(Corpus::read(&root).unwrap().policies.is_empty());
        assert_eq!(Corpus::read(&root).unwrap().patterns(), 0);
    }
}
