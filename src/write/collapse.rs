//! Folding a run of local versions into one, based on the version the tracked Vault records.
//!
//! A fold keeps the first version of each run, gives it the content and the words of the run's last
//! version, and removes the versions between. The versions a run keeps are its own rows, so a merge
//! kept as a checkpoint or a version named with `--keep` stands as it is. Nothing is rewritten on
//! disk: a file holding changes has only its baseline moved, which is the same rule `tweak` follows.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use rorolala_layout::LayoutPath;
use rorolala_utils_progress::Progress;
use rorolala_version_search::{TargetMiss, short};
use uuid::Uuid;

use crate::status::upstream_version;
use crate::write::journal::Journal;
use crate::write::tweak::{self, Chain, Mark, Request, Row, Sheet, TweakRefusal};
use crate::write::{Cancellation, WriteError};

/// What a collapse is told: where to stop folding, what to keep, and whether merges are kept.
#[derive(Debug, Clone, Default)]
pub struct CollapseOptions {
    /// The first version folded, when a run names one; the version above the origin otherwise.
    pub target: Option<String>,
    /// Versions to keep as they are, by a target a run writes.
    pub keep: Vec<String>,
    /// Whether a merge version in the range is kept as a checkpoint.
    pub keep_join: bool,
}

/// What a collapse came to.
#[derive(Debug, Clone)]
pub struct CollapseOutcome {
    /// The version the entry recorded before the fold, as hex.
    pub from: String,
    /// The version the entry records after the fold, as hex.
    pub to: String,
    /// How many versions the fold removed.
    pub folded: usize,
}

/// Why a chain could not be folded.
#[derive(Debug)]
pub enum CollapseRefusal {
    /// The path names no entry of the Layout.
    NotRecorded,
    /// There is no local version above the origin to fold.
    Nothing,
    /// The version the entry records is not the top of a chain.
    NotAChain,
    /// A variant waits to be joined into the entry.
    Merging {
        /// The variant waiting, as hex.
        variant: String,
    },
    /// A target names no version, or more than one.
    Target(TargetMiss),
    /// The sheet the fold built does not read against the chain.
    Sheet(String),
    /// The store, the index or the Layout refused.
    Failed(WriteError),
}

impl fmt::Display for CollapseRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRecorded => write!(formatter, "the Layout records no entry"),
            Self::Nothing => write!(formatter, "there is nothing to fold"),
            Self::NotAChain => write!(formatter, "the version is not the top of a chain"),
            Self::Merging { variant } => {
                write!(formatter, "the variant {variant} waits to be joined")
            }
            Self::Target(miss) => write!(formatter, "{miss}"),
            Self::Sheet(reason) => write!(formatter, "the sheet cannot be read: {reason}"),
            Self::Failed(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for CollapseRefusal {}

impl From<TweakRefusal> for CollapseRefusal {
    fn from(refusal: TweakRefusal) -> Self {
        match refusal {
            TweakRefusal::NotRecorded => Self::NotRecorded,
            TweakRefusal::Nothing => Self::Nothing,
            TweakRefusal::NotAChain => Self::NotAChain,
            TweakRefusal::Merging { variant } => Self::Merging { variant },
            TweakRefusal::Target(miss) => Self::Target(miss),
            TweakRefusal::Sheet { reason } => Self::Sheet(reason),
            TweakRefusal::Failed(error) => Self::Failed(error),
        }
    }
}

impl From<TargetMiss> for CollapseRefusal {
    fn from(miss: TargetMiss) -> Self {
        Self::Target(miss)
    }
}

impl From<WriteError> for CollapseRefusal {
    fn from(error: WriteError) -> Self {
        Self::Failed(error)
    }
}

/// Folds the local versions above the origin into one, keeping the content of each run's last.
///
/// A run is the versions between two kept ones, or between the lower bound and the top. Its first
/// version stays and takes the last version's creator, message and content; the versions between are
/// removed. The version the tracked Vault records is the base, or the root when there is none, so a
/// chain with no upstream still folds onto a fresh start.
///
/// # Errors
///
/// Returns [`CollapseRefusal`] when the path is not recorded, the target names no version, a variant
/// waits to be merged, or the store, the index or the Layout refuses.
pub fn collapse(
    request: &Request<'_>,
    id: Uuid,
    path: &LayoutPath,
    options: &CollapseOptions,
    progress: &Progress,
    cancellation: &Cancellation,
    journal: &mut Journal,
) -> Result<CollapseOutcome, CollapseRefusal> {
    let Some(index) = request.index else {
        return Err(WriteError::NoIndex.into());
    };
    let chain = tweak::chain_of(index, request.layout, path)?;
    let upstream = upstream_version(request.workspace, id);
    let origin = upstream.and_then(|upstream| tweak::origin_number(&chain, &upstream));
    let top = chain.top()?;

    let lower = match options.target.as_deref() {
        Some(target) => tweak::resolve(&chain, Some(target))?,
        None => origin.map_or(0, |origin| origin + 1),
    };
    if lower > top {
        return Err(CollapseRefusal::Nothing);
    }

    let kept = kept_versions(&chain, lower, top, options)?;
    let sheet = sheet_of(&chain, lower, top, &kept)?;
    let folded = sheet
        .rows()
        .iter()
        .filter(|row| row.mark == Mark::Remove)
        .count();
    let outcome = tweak::apply_with(
        request,
        id,
        path,
        &sheet,
        &tweak::ApplyOptions::default(),
        progress,
        cancellation,
        Some(journal),
    )?;

    Ok(CollapseOutcome {
        from: outcome.from,
        to: outcome.to,
        folded,
    })
}

/// The versions in `lower..=top` that are kept as they are.
fn kept_versions(
    chain: &Chain,
    lower: usize,
    top: usize,
    options: &CollapseOptions,
) -> Result<BTreeSet<usize>, CollapseRefusal> {
    let mut kept = BTreeSet::new();

    for target in &options.keep {
        let number = tweak::resolve(chain, Some(target))?;
        if (lower..=top).contains(&number) {
            kept.insert(number);
        }
    }

    if options.keep_join {
        for number in lower..=top {
            if chain.level(number)?.variant.join().is_some() {
                kept.insert(number);
            }
        }
    }

    Ok(kept)
}

/// The sheet `lower..=top` is folded as, with each run's first row carrying its last version.
fn sheet_of(
    chain: &Chain,
    lower: usize,
    top: usize,
    kept: &BTreeSet<usize>,
) -> Result<Sheet, CollapseRefusal> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut start = lower;
    for keep in kept.iter().copied() {
        if start < keep {
            runs.push((start, keep - 1));
        }
        start = keep + 1;
    }
    if start <= top {
        runs.push((start, top));
    }

    let collapsed: BTreeMap<usize, usize> = runs.into_iter().collect();
    let mut rows = Vec::with_capacity(top - lower + 1);
    for number in (lower..=top).rev() {
        if kept.contains(&number) {
            rows.push(Row::kept(chain.level(number)?));
        } else if let Some(last) = collapsed.get(&number) {
            let last = chain.level(*last)?;
            rows.push(Row {
                mark: Mark::Kept,
                creator: last.creator.clone(),
                message: last.message.clone(),
                content: short(last.variant.storage_hash()),
                join: None,
            });
        } else {
            rows.push(Row {
                mark: Mark::Remove,
                creator: String::new(),
                message: String::new(),
                content: String::new(),
                join: None,
            });
        }
    }

    Ok(Sheet::from_rows(chain, lower, rows)?)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use rorolala_layout::LayoutPath;
    use rorolala_storage::Key;
    use rorolala_utils_constants::VAULT_LAYOUT_NAME;
    use rorolala_utils_location::Locate as _;
    use rorolala_utils_progress::Progress;
    use rorolala_vcs::{VCSIndex, VCSWrite as _};
    use rorolala_workspace::Workspace;

    use super::{CollapseOptions, CollapseOutcome, collapse};
    use crate::write::Cancellation;
    use crate::write::Journal;
    use crate::write::track::{TrackOptions, track_file};
    use crate::write::tweak::{Chain, Request};

    /// A Workspace holding `a.txt`, recorded once per pair of content and message.
    fn recorded(name: &str, versions: &[(&str, &str)]) -> (PathBuf, Workspace) {
        let dir =
            std::env::temp_dir().join(format!("rola-write-collapse-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Workspace::create(&dir).unwrap();
        let workspace = Workspace::locate(&dir).unwrap();

        for (content, message) in versions {
            record(&workspace, &dir, content, message);
        }

        (dir, workspace)
    }

    /// Records `content` as the next version of `a.txt`.
    fn record(workspace: &Workspace, dir: &std::path::Path, content: &str, message: &str) {
        fs::write(dir.join("a.txt"), content).unwrap();
        track_file(
            workspace,
            dir,
            "a.txt",
            "alice",
            message,
            TrackOptions::default(),
        )
        .unwrap();
    }

    /// Publishes the Layout as the tracked Vault's fetched copy, at the version it records now.
    fn publish_copy(workspace: &Workspace) {
        workspace.set_layout_track("vault").unwrap();
        let dir = workspace.readonly_layouts_dir("vault", VAULT_LAYOUT_NAME);
        fs::create_dir_all(&dir).unwrap();
        let layout = workspace.layout().unwrap();

        for (name, bytes) in layout.as_files().unwrap() {
            fs::write(dir.join(name), bytes).unwrap();
        }
    }

    /// The chain `a.txt` records.
    fn chain_of(workspace: &Workspace) -> Chain {
        let layout = workspace.layout().unwrap();
        let path = LayoutPath::from_relative("a.txt".as_ref()).unwrap();
        let id = layout.id_of(&path).unwrap();
        let data = layout.entry(id).unwrap();
        let index = VCSIndex::locate(workspace.get_root()).unwrap();
        let version = index
            .read_version(&Key::new(data.version()).to_string())
            .unwrap();

        Chain::read(&index, &version).unwrap()
    }

    /// The version `a.txt` records.
    fn recorded_version(workspace: &Workspace) -> [u8; 32] {
        let layout = workspace.layout().unwrap();
        let path = LayoutPath::from_relative("a.txt".as_ref()).unwrap();
        let id = layout.id_of(&path).unwrap();

        layout.entry(id).unwrap().version()
    }

    /// Folds `a.txt` with `options`.
    fn collapsed(
        workspace: &Workspace,
        options: &CollapseOptions,
    ) -> Result<CollapseOutcome, super::CollapseRefusal> {
        let layout = workspace.layout().unwrap();
        let path = LayoutPath::from_relative("a.txt".as_ref()).unwrap();
        let id = layout.id_of(&path).unwrap();
        let index = VCSIndex::locate(workspace.get_root()).unwrap();
        let store = workspace.storage_set().unwrap();
        let candidates: Vec<(String, rorolala_protocol::VaultAddress)> = Vec::new();
        let request = Request {
            workspace,
            layout: &layout,
            index: Some(&index),
            store: Some(&store),
            account: None,
            candidates: &candidates,
            offline: false,
            concurrency: 1,
        };

        collapse(
            &request,
            id,
            &path,
            options,
            &Progress::silent(),
            &Cancellation::new(),
            &mut Journal::open(workspace, "collapse"),
        )
    }

    #[test]
    fn six_versions_fold_to_one_based_on_the_origin() {
        let (dir, workspace) = recorded(
            "six",
            &[("one", "first"), ("two", "second"), ("three", "third")],
        );
        publish_copy(&workspace);
        for (content, message) in [("four", "fourth"), ("five", "fifth"), ("six", "sixth")] {
            record(&workspace, &dir, content, message);
        }

        let chain = chain_of(&workspace);
        let origin = *chain.level(2).unwrap().version.hash().digest();
        let top = *chain.level(5).unwrap().variant.storage_hash();
        let outcome = collapsed(&workspace, &CollapseOptions::default()).unwrap();
        let index = VCSIndex::locate(workspace.get_root()).unwrap();
        let version = index.read_version(&outcome.to).unwrap();
        let variant = index
            .read_variant(&Key::new(*version.variant()).to_string())
            .unwrap();

        assert_eq!(outcome.folded, 2);
        assert_eq!(*variant.base_version(), origin);
        assert_eq!(*variant.storage_hash(), top);
        assert_eq!(recorded_version(&workspace), *version.hash().digest());
        assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "six");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_kept_version_stays_as_a_checkpoint() {
        let (dir, workspace) = recorded(
            "keep",
            &[("one", "first"), ("two", "second"), ("three", "third")],
        );
        publish_copy(&workspace);
        for (content, message) in [("four", "fourth"), ("five", "fifth"), ("six", "sixth")] {
            record(&workspace, &dir, content, message);
        }

        let chain = chain_of(&workspace);
        let checkpoint = *chain.level(3).unwrap().version.hash().digest();
        let options = CollapseOptions {
            keep: vec!["~2".to_owned()],
            ..CollapseOptions::default()
        };
        let outcome = collapsed(&workspace, &options).unwrap();
        let index = VCSIndex::locate(workspace.get_root()).unwrap();
        let version = index.read_version(&outcome.to).unwrap();
        let variant = index
            .read_variant(&Key::new(*version.variant()).to_string())
            .unwrap();

        assert_eq!(outcome.folded, 1);
        assert_eq!(*variant.base_version(), checkpoint);
        assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "six");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_changed_file_is_left_alone() {
        let (dir, workspace) = recorded(
            "dirty",
            &[("one", "first"), ("two", "second"), ("three", "third")],
        );
        publish_copy(&workspace);
        for (content, message) in [("four", "fourth"), ("five", "fifth"), ("six", "sixth")] {
            record(&workspace, &dir, content, message);
        }
        fs::write(dir.join("a.txt"), "local edit").unwrap();

        let outcome = collapsed(&workspace, &CollapseOptions::default()).unwrap();

        assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "local edit");
        assert_eq!(Key::new(recorded_version(&workspace)).hex(), outcome.to);

        let _ = fs::remove_dir_all(dir);
    }
}
