//! Cutting the old history off a file, and starting a new file at a kept version.
//!
//! A cut replays the versions from the cut point upward onto the root, so the new chain holds only
//! the work from that point and every version keeps its content, creator, message and merge. The
//! entry is given a fresh `Uuid` unless a run asks to keep it, which is what starts a new file rather
//! than a fork. Nothing is rewritten on disk: the content is the same, and a changed file has only
//! its baseline moved.

use std::fmt;
use std::str::FromStr as _;

use rorolala_layout::LayoutPath;
use rorolala_storage::Key;
use rorolala_utils_progress::Progress;
use rorolala_vcs::{VCSWrite as _, Version};
use rorolala_version_search::TargetMiss;
use uuid::Uuid;

use crate::status::upstream_version;
use crate::write::journal::{Effect, EntryState, Journal};
use crate::write::layout::switch_entry;
use crate::write::tweak::{self, ApplyOptions, Request, Row, Sheet, TweakRefusal};
use crate::write::{Cancellation, WriteError};

/// What a cut is told: where to cut, whether the entry keeps its `Uuid`, and what happens to merges.
#[derive(Debug, Clone, Default)]
pub struct CutOptions {
    /// The first version kept in the new chain, when a run names one.
    pub target: Option<String>,
    /// Whether the entry keeps its `Uuid`, which forks it from the Vault.
    pub keep_uuid: bool,
    /// Whether every merge in the kept range is dropped.
    pub strip_join: bool,
}

/// What a cut came to.
#[derive(Debug, Clone)]
pub struct CutOutcome {
    /// The version the entry recorded before the cut, as hex.
    pub from: String,
    /// The version the entry records after the cut, as hex.
    pub to: String,
    /// The first version the new chain keeps, as a name for a person.
    pub point: String,
    /// The `Uuid` the path records after the cut.
    pub uuid: String,
    /// How many versions the new chain keeps.
    pub kept: usize,
}

/// Why a chain could not be cut.
#[derive(Debug)]
pub enum CutRefusal {
    /// The path names no entry of the Layout.
    NotRecorded,
    /// The chain holds no version to cut.
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
    /// The sheet the cut built does not read against the chain.
    Sheet(String),
    /// The store, the index or the Layout refused.
    Failed(WriteError),
}

impl fmt::Display for CutRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRecorded => write!(formatter, "the Layout records no entry"),
            Self::Nothing => write!(formatter, "the chain holds no version to cut"),
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

impl std::error::Error for CutRefusal {}

impl From<TweakRefusal> for CutRefusal {
    fn from(refusal: TweakRefusal) -> Self {
        match refusal {
            TweakRefusal::NotRecorded => Self::NotRecorded,
            TweakRefusal::Nothing => Self::Nothing,
            TweakRefusal::NotAChain => Self::NotAChain,
            TweakRefusal::Merging { variant } => Self::Merging { variant },
            TweakRefusal::Target(miss) => Self::Target(miss),
            TweakRefusal::Sheet { reason } => Self::Sheet(reason.to_string()),
            TweakRefusal::Failed(error) => Self::Failed(error),
        }
    }
}

impl From<TargetMiss> for CutRefusal {
    fn from(miss: TargetMiss) -> Self {
        Self::Target(miss)
    }
}

impl From<WriteError> for CutRefusal {
    fn from(error: WriteError) -> Self {
        Self::Failed(error)
    }
}

/// Cuts the chain at a kept version, replays it onto the root, and gives the path a fresh `Uuid`.
///
/// The kept versions are based on the root in turn, each carrying what it carried before. A merge is
/// kept unless `strip_join` is set, in which case the merge is cleared and the joined variants are not
/// replayed. A changed file has only its baseline moved, which is the rule every history operation
/// follows.
///
/// # Errors
///
/// Returns [`CutRefusal`] when the path is not recorded, the target names no version, a variant waits
/// to be merged, or the store, the index or the Layout refuses.
pub fn cut(
    request: &Request<'_>,
    id: Uuid,
    path: &LayoutPath,
    options: &CutOptions,
    progress: &Progress,
    cancellation: &Cancellation,
    journal: &mut Journal,
) -> Result<CutOutcome, CutRefusal> {
    let Some(index) = request.index else {
        return Err(WriteError::NoIndex.into());
    };
    let chain = tweak::chain_of(index, request.layout, path)?;
    let upstream = upstream_version(request.workspace, id);
    let origin = upstream.and_then(|upstream| tweak::origin_number(&chain, &upstream));
    let top = chain.top()?;

    let point = match options.target.as_deref() {
        Some(target) => tweak::resolve(&chain, Some(target))?,
        None => origin.map_or(0, |origin| origin + 1).min(top),
    };

    let rows = (point..=top)
        .rev()
        .map(|number| {
            let mut row = Row::kept(chain.level(number)?);
            if options.strip_join {
                row.join = None;
            }

            Ok(row)
        })
        .collect::<Result<Vec<_>, TweakRefusal>>()?;
    let sheet = Sheet::from_rows(&chain, point, rows)?;
    let root = *Version::root().hash().digest();
    let outcome = tweak::apply_with(
        request,
        id,
        path,
        &sheet,
        &ApplyOptions {
            base: Some(root),
            replay_based: !options.strip_join,
        },
        progress,
        cancellation,
        Some(journal),
    )?;

    let kept = top - point + 1;
    let named = chain.name(point)?;
    if options.keep_uuid {
        return Ok(CutOutcome {
            from: outcome.from,
            to: outcome.to,
            point: named,
            uuid: id.to_string(),
            kept,
        });
    }

    let new_id = Uuid::new_v4();
    let version = *Key::from_str(&outcome.to)
        .map_err(|error| WriteError::Internal {
            reason: error.to_string(),
        })?
        .digest();
    let entry = request.layout.entry(id).ok_or(CutRefusal::NotRecorded)?;
    let data = entry.renewed(entry.owner().map(str::to_owned), version);
    journal.record(Effect::Uuid {
        from: id,
        to: new_id,
        path: Some(path.as_str().to_owned()),
        before: EntryState::of(&entry),
        after: EntryState::of(&data),
    });
    switch_entry(request.layout, path, new_id, data)?;

    Ok(CutOutcome {
        from: outcome.from,
        to: outcome.to,
        point: named,
        uuid: new_id.to_string(),
        kept,
    })
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
    use rorolala_vcs::{VCSIndex, VCSWrite as _, Version};
    use rorolala_workspace::Workspace;
    use uuid::Uuid;

    use super::{CutOptions, CutOutcome, cut};
    use crate::write::Cancellation;
    use crate::write::Journal;
    use crate::write::tweak::{Chain, Request};
    use crate::write::track::{TrackOptions, track_file};

    /// A Workspace holding `a.txt`, recorded once per pair of content and message.
    fn recorded(name: &str, versions: &[(&str, &str)]) -> (PathBuf, Workspace) {
        let dir =
            std::env::temp_dir().join(format!("rola-write-cut-{name}-{}", std::process::id()));
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

    /// The `Uuid` `a.txt` records.
    fn recorded_id(workspace: &Workspace) -> Uuid {
        let layout = workspace.layout().unwrap();
        let path = LayoutPath::from_relative("a.txt".as_ref()).unwrap();

        layout.id_of(&path).unwrap()
    }

    /// The version `a.txt` records.
    fn recorded_version(workspace: &Workspace) -> [u8; 32] {
        let layout = workspace.layout().unwrap();
        let path = LayoutPath::from_relative("a.txt".as_ref()).unwrap();
        let id = layout.id_of(&path).unwrap();

        layout.entry(id).unwrap().version()
    }

    /// Cuts `a.txt` with `options`.
    fn cut_of(
        workspace: &Workspace,
        options: &CutOptions,
    ) -> Result<CutOutcome, super::CutRefusal> {
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

        cut(
            &request,
            id,
            &path,
            options,
            &Progress::silent(),
            &Cancellation::new(),
            &mut Journal::open(workspace, "cut"),
        )
    }

    #[test]
    fn six_versions_cut_at_the_origin_start_a_new_file() {
        let (dir, workspace) = recorded(
            "default",
            &[("one", "first"), ("two", "second"), ("three", "third")],
        );
        publish_copy(&workspace);
        for (content, message) in [("four", "fourth"), ("five", "fifth"), ("six", "sixth")] {
            record(&workspace, &dir, content, message);
        }

        let chain = chain_of(&workspace);
        let top = *chain.level(5).unwrap().variant.storage_hash();
        let old = recorded_id(&workspace);
        let outcome = cut_of(&workspace, &CutOptions::default()).unwrap();

        assert_eq!(outcome.kept, 3);
        assert_ne!(outcome.uuid, old.to_string());
        assert_eq!(Key::new(recorded_version(&workspace)).hex(), outcome.to);
        assert!(workspace.layout().unwrap().entry(old).is_none());

        let index = VCSIndex::locate(workspace.get_root()).unwrap();
        let version = index.read_version(&outcome.to).unwrap();
        let variant = index
            .read_variant(&Key::new(*version.variant()).to_string())
            .unwrap();
        assert_eq!(*variant.storage_hash(), top);

        // Every kept version is based on the one below it, and the first is based on the root.
        let new_chain = Chain::read(&index, &version).unwrap();
        assert_eq!(new_chain.len(), 3);
        assert_eq!(
            *new_chain.level(0).unwrap().variant.base_version(),
            *Version::root().hash().digest()
        );
        assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "six");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn keeping_the_uuid_keeps_the_entry() {
        let (dir, workspace) = recorded(
            "keep-uuid",
            &[("one", "first"), ("two", "second"), ("three", "third")],
        );
        publish_copy(&workspace);
        for (content, message) in [("four", "fourth"), ("five", "fifth"), ("six", "sixth")] {
            record(&workspace, &dir, content, message);
        }
        let old = recorded_id(&workspace);
        let options = CutOptions {
            keep_uuid: true,
            ..CutOptions::default()
        };
        let outcome = cut_of(&workspace, &options).unwrap();

        assert_eq!(outcome.uuid, old.to_string());
        assert_eq!(Key::new(recorded_version(&workspace)).hex(), outcome.to);
        assert!(workspace.layout().unwrap().entry(old).is_some());

        let _ = fs::remove_dir_all(dir);
    }
}
