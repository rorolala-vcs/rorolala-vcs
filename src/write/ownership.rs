//! Reading the gates a hold or a giveup has to pass before any ownership moves.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use rorolala_auth::Account;
use rorolala_ignore::IgnoreRuleSet;
use rorolala_layout::{EntryKind, Layout, LayoutPath, MutableData};
use rorolala_protocol::VaultAddress;
use rorolala_tree::Tree;
use rorolala_utils_constants::WORKSPACE_IGNORE_RULES_DIR;
use rorolala_utils_lazyffi::{StrSpan, lazyffi};
use rorolala_utils_location::{Locate as _, normalize};
use rorolala_utils_progress::Progress;
use rorolala_workspace::Workspace;
use rorolala_workspace_analyze::{TreeDiff, tree_diff, walk};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::daemon::{
    Ownership, OwnershipChange, action_fetch_layout, action_giveup_ownership_async,
    action_request_ownership_async,
};
use crate::write::layout::{cached_copy, set_cached_owners};
use crate::write::request::{Common, WriteCommon};
use crate::write::{Cancellation, WriteError, WriteProgress};

/// How alike two text files have to be to count as the same file moved.
///
/// The reading is consulted for a giveup, where work the Layout does not name may not be handed
/// over, and it is the same likeness a command reads a tree with, so a plan made through a request
/// and one made by a command agree.
const ALIKE: f32 = 0.6;

/// Which way an entry's ownership is to move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Take the entry for this account.
    Hold,
    /// Let the entry go.
    Giveup,
}

/// What a run may do beside taking or letting go.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// Go past a version the Vault no longer names.
    pub force: bool,
}

/// One path a run named, and whether it named it outright.
#[derive(Debug, Clone)]
pub struct Named {
    /// Where it sits in the Layout.
    pub path: LayoutPath,
    /// Whether the run named this path itself rather than a directory over it.
    pub direct: bool,
}

/// One entry a run is about to move.
#[derive(Debug, Clone)]
pub struct Wanted {
    /// Where it sits in the Layout.
    pub path: LayoutPath,
    /// The `Uuid` ownership is about.
    pub id: Uuid,
}

/// Why an entry's ownership may not be moved.
///
/// The serde shape is stable for the C ABI: an externally tagged object whose tag is the variant in
/// `snake_case`, so a path refusal is `{"path":"a/b.psd"}` and a taken one is
/// `{"taken":{"path":"a/b.psd","owner":"bob"}}`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    /// The Layout names nothing at the path.
    Path(String),
    /// The Vault's Layout names nothing for the `Uuid`.
    Upstream(String),
    /// What is here is not the version the Vault names.
    Version(String),
    /// What is here has changes the Layout does not name.
    Changed(String),
    /// The entry is held by another account.
    Taken {
        /// The path.
        path: String,
        /// The account that holds it.
        owner: String,
    },
    /// The entry is not this account's to let go of.
    NotHeld {
        /// The path.
        path: String,
        /// The account that holds it, if any.
        owner: Option<String>,
    },
    /// The exchange could not be made.
    Failed(String),
}

/// One entry whose ownership the Vault agreed to move.
///
/// Its serde shape is what a C caller reads: `{"path":"a/b.psd","uuid":"...","owner":"bob"}`,
/// with `owner` null when nobody holds the entry.
#[derive(Debug, Clone, Serialize)]
pub struct Item {
    /// The path the entry is named at.
    pub path: String,
    /// The entry's `Uuid`.
    pub uuid: String,
    /// The owner the Vault answered with.
    pub owner: Option<String>,
}

/// What a moving run came to.
///
/// Its serde shape is what a C caller reads: `entries` is an array of [`Item`] and `failed` is an
/// array of [`Refusal`], both in the order the run read them.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Outcome {
    /// The entries the Vault moved.
    pub entries: Vec<Item>,
    /// What was refused.
    pub failed: Vec<Refusal>,
}

/// Moves the ownership of everything the gates allowed.
///
/// The whole batch is one exchange, over one connection. What the Vault agreed to is written into
/// `copy` in one batch afterwards. `name` is what the exchange's progress bar is drawn under, in the
/// caller's language: the Vault's side has no words of its own.
///
/// # Errors
///
/// Returns [`WriteError::Cancelled`] when `cancellation` was set before the exchange began. The
/// exchange itself is one handshake and is not interrupted between entries: stopping partway through
/// would leave the Vault's answer and this side's copy disagreeing.
#[allow(
    clippy::too_many_arguments,
    reason = "the run needs both Layouts, the account it signs as, the Vault it reaches and the watching"
)]
pub fn apply(
    kind: Kind,
    wanted: Vec<Wanted>,
    workspace: &Workspace,
    copy: &Layout,
    account: &Account,
    target: &VaultAddress,
    name: &str,
    progress: &Progress,
    cancellation: &Cancellation,
) -> Result<Outcome, WriteError> {
    if cancellation.is_cancelled() {
        return Err(WriteError::Cancelled);
    }

    if wanted.is_empty() {
        return Ok(Outcome::default());
    }

    let ids: Vec<String> = wanted.iter().map(|item| item.id.to_string()).collect();
    let change = OwnershipChange {
        name: name.to_owned(),
        uuids: ids,
    };
    let target = target.to_string();
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        return Ok(Outcome {
            entries: Vec::new(),
            failed: vec![Refusal::Failed("the runtime could not be made".to_owned())],
        });
    };

    let answer = runtime.block_on(async {
        match kind {
            Kind::Hold => {
                action_request_ownership_async(
                    workspace,
                    account,
                    target.clone(),
                    change.clone(),
                    progress.clone(),
                )
                .await
            }
            Kind::Giveup => {
                action_giveup_ownership_async(
                    workspace,
                    account,
                    target.clone(),
                    change.clone(),
                    progress.clone(),
                )
                .await
            }
        }
    });

    let outcomes: Vec<Ownership> = match answer.map_err(|error| Refusal::Failed(error.to_string()))
    {
        Ok(answer) => answer.outcomes(),
        Err(refusal) => {
            return Ok(Outcome {
                entries: Vec::new(),
                failed: vec![refusal],
            });
        }
    };

    let mut entries = Vec::with_capacity(wanted.len());
    let mut failed = Vec::new();
    let mut changes = Vec::with_capacity(wanted.len());

    for (item, outcome) in wanted.into_iter().zip(outcomes) {
        match outcome {
            Ownership::Owner(owner) => {
                changes.push((item.id, owner.clone()));
                entries.push(Item {
                    path: item.path.as_str().to_owned(),
                    uuid: item.id.to_string(),
                    owner,
                });
            }
            Ownership::Missing => failed.push(Refusal::Upstream(item.path.as_str().to_owned())),
            Ownership::HeldBy(owner) => failed.push(Refusal::Taken {
                path: item.path.as_str().to_owned(),
                owner,
            }),
        }
    }

    let mut batch = copy.batch();
    if let Err(error) = set_cached_owners(&mut batch, &changes).and_then(|()| batch.commit()) {
        failed.push(Refusal::Failed(error.to_string()));
    }

    Ok(Outcome { entries, failed })
}

/// Reads every gate for every path, and says what may move and what may not.
///
/// Nothing is moved here: a batch is judged whole before any of it is done.
///
/// # Errors
///
/// Returns [`WriteError::Cancelled`] when `cancellation` was set before or between the paths.
#[allow(
    clippy::too_many_arguments,
    reason = "every gate the run reads is a value it has to be handed: both Layouts, the tree reading, the rules and the watching"
)]
pub fn plan(
    kind: Kind,
    options: Options,
    workspace: &Workspace,
    layout: &Layout,
    copy: &Layout,
    me: &str,
    named: &[Named],
    diff: Option<&TreeDiff>,
    rules: &IgnoreRuleSet,
    cancellation: &Cancellation,
) -> Result<(Vec<Wanted>, Vec<Refusal>, usize), WriteError> {
    if cancellation.is_cancelled() {
        return Err(WriteError::Cancelled);
    }

    let mut wanted = Vec::new();
    let mut failed = Vec::new();
    let mut skipped = 0;

    for Named { path, direct } in named {
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled);
        }

        let Some(id) = layout.id_of(path) else {
            if *direct {
                failed.push(Refusal::Path(path.as_str().to_owned()));
            } else {
                skipped += 1;
            }

            continue;
        };

        let Some(upstream) = copy.entry(id) else {
            failed.push(Refusal::Upstream(path.as_str().to_owned()));

            continue;
        };

        let settled = if options.force {
            Ok(())
        } else {
            diff.map_or(Ok(()), |diff| {
                settled(kind, workspace, path, layout, &upstream, diff, rules)
            })
        };

        match settled.and_then(|()| free(kind, path, &upstream, me)) {
            Ok(()) => wanted.push(Wanted {
                path: path.clone(),
                id,
            }),
            Err(refusal) => failed.push(refusal),
        }
    }

    Ok((wanted, failed, skipped))
}

/// The paths the names stand for.
///
/// A name that is a file is that file; one that is a directory is every file under it, read the way
/// the tree reading reads the tree, so what the Workspace keeps for itself is left out. A name
/// that is both is one path.
///
/// # Errors
///
/// Returns the name that does not read, as it was given.
pub fn expand(workspace: &Workspace, cwd: &Path, names: &[String]) -> Result<Vec<Named>, String> {
    let root = normalize(workspace.get_root());
    let mut paths: BTreeMap<LayoutPath, bool> = BTreeMap::new();
    let mut walked = None;

    for given in names {
        let disk = normalize(&resolve(cwd, given));

        let relative = disk
            .strip_prefix(&root)
            .map_err(|_| given.clone())?
            .to_path_buf();

        if let Some(tree) = tree_under(&root, &disk) {
            paths.insert(tree, true);
            continue;
        }

        if disk.is_file() {
            let path = LayoutPath::from_relative(&relative).map_err(|_| given.clone())?;
            paths.insert(path, true);
            continue;
        }

        if !disk.is_dir() {
            return Err(given.clone());
        }

        let found = walked.get_or_insert_with(|| walk(&root).0);
        let prefix = if relative.as_os_str().is_empty() {
            String::new()
        } else {
            LayoutPath::from_relative(&relative)
                .map_err(|_| given.clone())?
                .as_str()
                .to_owned()
        };

        for file in found.iter() {
            if !matches!(file.kind, EntryKind::File | EntryKind::Tree) {
                continue;
            }

            if under(&prefix, &file.path) {
                paths.entry(file.path.clone()).or_insert(false);
            }
        }
    }

    Ok(paths
        .into_iter()
        .map(|(path, direct)| Named { path, direct })
        .collect())
}

/// The tree a path is in, when it is in one.
fn tree_under(root: &Path, disk: &Path) -> Option<LayoutPath> {
    let tree = Tree::locate(root, disk).ok()?;

    tree.layout_path().ok()
}

/// Which way an ownership request moves.
///
/// It is the request layer's spelling of [`Kind`]: the fields a C caller fills carry the kind the
/// library writes, while the plan and the apply take the same value.
///
/// # FFI
///
/// A `RolaWriteOwnershipKind` the caller fills, one `tag` naming which way the ownership moves.
#[lazyffi(export = RolaWriteOwnershipKind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipKind {
    /// Take the entry for this account.
    Hold,
    /// Let the entry go.
    Giveup,
}

impl From<OwnershipKind> for Kind {
    fn from(kind: OwnershipKind) -> Self {
        match kind {
            OwnershipKind::Hold => Self::Hold,
            OwnershipKind::Giveup => Self::Giveup,
        }
    }
}

/// A request to move ownership, as JSON names it.
///
/// It is everything the request layer reads: the fields every write request carries, which way the
/// ownership moves, the names to move it for, and whether a batch that cannot be done whole is done
/// in part. The order it resolves them in is the command line's order, so a C caller and a Rust
/// caller meet the same gates.
#[derive(Debug, Clone, Deserialize)]
pub struct OwnershipRequest {
    /// The Workspace, the account, the Vault and the words every write request carries.
    #[serde(flatten)]
    pub common: Common,
    /// Which way the ownership moves.
    pub kind: OwnershipKind,
    /// The names to move the ownership for.
    ///
    /// A name is read from the Workspace's root, since a request has no directory of its own the
    /// way a command has a working directory: a relative name is work inside the Workspace, and an
    /// absolute one is taken as it stands. A file is itself, a directory is every file under it,
    /// and a request that names none plans nothing.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Whether what can be moved is moved when not every name passes its gates.
    #[serde(default)]
    pub allow_partial: bool,
}

/// An ownership request a C caller fills.
///
/// # FFI
///
/// A `RolaWriteOwnershipRequest` the caller fills. Every `const char *` is borrowed for the call
/// and released by the caller, so nothing here is freed when the call returns. `paths` is a
/// `RolaStrSpan` of the names to move the ownership for, each read from the Workspace root.
#[lazyffi(export = RolaWriteOwnershipRequest, input)]
#[derive(Debug, Clone)]
pub struct OwnershipInput {
    /// The Workspace, the account, the Vault and the words every write request carries.
    pub common: WriteCommon,
    /// Which way the ownership moves.
    pub kind: OwnershipKind,
    /// The names to move the ownership for.
    pub paths: StrSpan,
    /// Whether what can be moved is moved when not every name passes its gates.
    pub allow_partial: bool,
}

impl From<OwnershipInput> for OwnershipRequest {
    fn from(input: OwnershipInput) -> Self {
        Self {
            common: input.common.into(),
            kind: input.kind,
            paths: input.paths.into_vec(),
            allow_partial: input.allow_partial,
        }
    }
}

/// One entry a plan promised to move, as an apply is handed it back.
///
/// # FFI
///
/// A `RolaWriteOwnershipAgreed` the caller fills from what a plan answered. Both strings are
/// borrowed for the call and released by the caller.
#[lazyffi(export = RolaWriteOwnershipAgreed, input)]
#[derive(Debug, Clone)]
pub struct OwnershipAgreed {
    /// Where the entry sits in the Layout.
    pub path: String,
    /// The `Uuid` ownership is about, as text.
    pub uuid: String,
}

impl From<OwnershipAgreed> for WantedView {
    fn from(input: OwnershipAgreed) -> Self {
        Self {
            path: input.path,
            uuid: input.uuid,
        }
    }
}

/// One entry a plan means to move, as JSON names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WantedView {
    /// Where the entry sits in the Layout.
    pub path: String,
    /// The `Uuid` ownership is about, as text.
    pub uuid: String,
}

impl From<Wanted> for WantedView {
    fn from(wanted: Wanted) -> Self {
        Self {
            path: wanted.path.as_str().to_owned(),
            uuid: wanted.id.to_string(),
        }
    }
}

/// What a plan would move, what it refused, and what a directory left out.
///
/// The serde shape is what a C caller reads: `wanted` is an array of
/// `{"path":"a/b.psd","uuid":"..."}`, `refused` is the shape [`Refusal`] writes, and `skipped` is
/// how many files a name that is a directory brought in that the Layout does not name.
#[derive(Debug, Clone, Serialize)]
pub struct OwnershipPlanReport {
    /// The entries a move would act on, in the plan's path order.
    pub wanted: Vec<WantedView>,
    /// Why the gates refuse an entry, in path order.
    pub refused: Vec<Refusal>,
    /// How many files a name that is a directory brought in that the Layout does not name.
    pub skipped: usize,
}

/// What a request resolved to, and what a plan made of it.
struct Prepared {
    /// The Workspace the request names.
    workspace: Workspace,
    /// The copy of the Vault's Layout the fetch left behind.
    copy: Layout,
    /// The account the request acts as.
    account: Account,
    /// Where the Vault answers.
    target: VaultAddress,
    /// The entries a move would act on.
    wanted: Vec<Wanted>,
    /// What the gates refused.
    refused: Vec<Refusal>,
    /// How many files a directory brought in that the Layout does not name.
    skipped: usize,
}

/// The plan an apply was handed, read only for what it promised to move.
///
/// The rest of the report is not read: what this run has to agree with is the entries, and the
/// words a refusal is told in are the caller's to keep.
impl OwnershipRequest {
    /// Reads every gate the request names, and says what may move and what may not.
    ///
    /// The Vault's Layout is fetched first, since every gate is read against what it holds now, and
    /// the copy the fetch leaves here is what ownership is judged against. Nothing is moved here: a
    /// batch is judged whole before any of it is done, and what this answers is the judgment.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Offline`] when the request is offline, since no plan can be made
    /// without the fetch; [`WriteError::NoWorkspace`], [`WriteError::NoLayout`],
    /// [`WriteError::NoAccount`], [`WriteError::NoTrack`] or [`WriteError::Precondition`] when the
    /// request does not resolve; [`WriteError::Action`] when the fetch fails;
    /// [`WriteError::NotCached`] when the fetch left no copy; [`WriteError::InvalidPath`] when a
    /// name is not a file or a directory inside the Workspace; [`WriteError::Layout`] when the
    /// reading does not resolve; and [`WriteError::Cancelled`] when `cancellation` was set.
    pub fn request_plan(
        &self,
        progress: &Progress,
        cancellation: &Cancellation,
    ) -> Result<OwnershipPlanReport, WriteError> {
        let prepared = self.prepare(progress, cancellation)?;

        Ok(OwnershipPlanReport {
            wanted: prepared.wanted.into_iter().map(WantedView::from).collect(),
            refused: prepared.refused,
            skipped: prepared.skipped,
        })
    }

    /// Moves the ownership a plan named, once every gate has been read again.
    ///
    /// The gates are read exactly as [`request_plan`](Self::request_plan) reads them, because the
    /// Workspace may have changed since the plan was made: when `plan_json` is given, the entries
    /// this run would move are compared with the ones it names, and a difference is refused rather
    /// than applied. When `allow_partial` is not set and a gate refused anything, nothing moves and
    /// the refusals are the answer, since a batch that cannot be done whole is not half done. What
    /// the Vault agrees to is written into the fetched copy afterwards, so a reading next shows the
    /// ownership as it now is.
    ///
    /// # Errors
    ///
    /// Returns what [`request_plan`](Self::request_plan) returns, [`WriteError::Precondition`] when
    /// `wanted` names entries this run would not move, and [`WriteError::Cancelled`] when
    /// `cancellation` was set.
    pub fn request_apply(
        &self,
        wanted: Option<&[WantedView]>,
        progress: &Progress,
        cancellation: &Cancellation,
    ) -> Result<Outcome, WriteError> {
        let prepared = self.prepare(progress, cancellation)?;

        if let Some(wanted) = wanted.filter(|wanted| !wanted.is_empty())
            && !same_entries(&prepared.wanted, wanted)
        {
            return Err(WriteError::Precondition {
                what: "the ownership plan".to_owned(),
                reason: "the entries to move are not the ones it names".to_owned(),
            });
        }

        if !self.allow_partial && !prepared.refused.is_empty() {
            return Ok(Outcome {
                entries: Vec::new(),
                failed: prepared.refused,
            });
        }

        let Prepared {
            workspace,
            copy,
            account,
            target,
            wanted,
            mut refused,
            ..
        } = prepared;
        let mut outcome = apply(
            Kind::from(self.kind),
            wanted,
            &workspace,
            &copy,
            &account,
            &target,
            &self.common.labels.applying,
            progress,
            cancellation,
        )?;

        refused.append(&mut outcome.failed);
        outcome.failed = refused;

        Ok(outcome)
    }

    /// Reads every gate once, in the order a command reads them.
    ///
    /// A plan and an apply both begin here rather than each reading the gates its own way: an apply
    /// re-reads them because the Workspace may have changed since the plan was made, and a second
    /// reading written apart would be a second set of rules to keep right.
    fn prepare(
        &self,
        progress: &Progress,
        cancellation: &Cancellation,
    ) -> Result<Prepared, WriteError> {
        // Every gate is read against what the Vault holds now, so the fetch is not a step a plan
        // can skip: a run told to stay offline has no plan to make.
        if self.common.offline {
            return Err(WriteError::Offline);
        }
        if cancellation.is_cancelled() {
            return Err(WriteError::Cancelled);
        }

        let workspace = self.common.workspace()?;
        let layout = self.common.layout(&workspace)?;
        let account = self.common.account(&workspace)?;
        let vault = self.common.vault(&workspace)?;

        // The fetch is the wait a reader watches here, and the gates after it are read on this
        // side: the bar covers both, so a reader is not shown a bar that appeared once the slow
        // half was over.
        let mut step = progress.begin(&self.common.labels.fetching, Some(2));

        action_fetch_layout(
            &workspace,
            &account,
            vault.address.to_string(),
            &crate::daemon::FetchLayoutRequest {
                name: vault.name.clone(),
            },
        )
        .map_err(|error| WriteError::Action {
            reason: error.to_string(),
        })?;
        step.advance_by(1);

        let copy = cached_copy(&workspace, &vault.name)
            .map_err(|error| WriteError::Layout {
                reason: error.to_string(),
            })?
            .ok_or(WriteError::NotCached)?;

        // A request has no working directory of its own the way a command has one, so a relative
        // name is read from the Workspace's root: work inside the Workspace is what a request can
        // name by definition, and a process directory would mean something different to every
        // caller. The root is normalized here so that the names and the tree reading agree on it.
        let root = normalize(workspace.get_root());
        let named =
            expand(&workspace, &root, &self.paths).map_err(|name| WriteError::InvalidPath {
                path: name,
                reason: "it is not a file or a directory inside the Workspace".to_owned(),
            })?;

        // The rules only govern paths the Layout does not name, and ownership is about entries it
        // does name; a rule set that cannot be read is answered as no rules, the way an empty
        // directory of them is.
        let rules = IgnoreRuleSet::load(&root.join(WORKSPACE_IGNORE_RULES_DIR), &root)
            .unwrap_or_else(|_| IgnoreRuleSet::empty());

        // A claim may be made over work written before it was locked, so only a giveup reads the
        // tree, and force moves past that reading, which is why it is not taken at all when forced.
        let diff = if self.common.force {
            None
        } else {
            Some(
                tree_diff(&layout, &workspace, ALIKE, &IgnoreRuleSet::empty()).map_err(
                    |error| WriteError::Layout {
                        reason: error.to_string(),
                    },
                )?,
            )
        };

        let (wanted, refused, skipped) = plan(
            Kind::from(self.kind),
            Options {
                force: self.common.force,
            },
            &workspace,
            &layout,
            &copy,
            &self.common.account,
            &named,
            diff.as_ref(),
            &rules,
            cancellation,
        )?;
        step.advance_by(1);

        Ok(Prepared {
            workspace,
            copy,
            account,
            target: vault.address,
            wanted,
            refused,
            skipped,
        })
    }
}

/// Whether a plan made now moves the entries `given` names.
///
/// The order is not compared, since it is the plan's own path order and a caller that reordered the
/// array still asked for the same entries. A `Uuid` is compared as a value rather than as text, so
/// the same entry written in another case is still the same entry.
fn same_entries(wanted: &[Wanted], given: &[WantedView]) -> bool {
    let now: BTreeSet<(String, Uuid)> = wanted
        .iter()
        .map(|item| (item.path.as_str().to_owned(), item.id))
        .collect();
    let then: BTreeSet<(String, Uuid)> = given
        .iter()
        .filter_map(|view| {
            Uuid::parse_str(&view.uuid)
                .ok()
                .map(|id| (view.path.clone(), id))
        })
        .collect();

    then.len() == given.len() && now == then
}

/// Plans the ownership moves a request names, without moving any.
///
/// # FFI
///
/// `request` is a `RolaWriteOwnershipRequest` the caller fills and keeps; every `const char *` in
/// it is borrowed for the call and released by the caller. A relative name in `paths` is read from
/// the Workspace's root, not from the process's working directory, and an absolute one is taken as
/// it stands.
///
/// A `RolaWriteOwnershipPlan *` the caller releases with `free_rola_write_ownership_plan` answers
/// it: `_wanted_count` and `_wanted_at` name each entry the move would act on, `_refused_count` and
/// `_refused_at` name why the gates refuse one, and `_skipped` is how many files a directory
/// brought in that the Layout does not name.
///
/// # Errors
///
/// Returns a `RolaWriteError *` the caller releases with `free_rola_write_error`. The tag is
/// [`WriteError`]'s: `Offline` when the request is offline, `NoWorkspace`, `NoLayout`, `NoAccount`,
/// `NoTrack` or `Precondition` when it does not resolve, `Action` when the fetch fails, `NotCached`
/// when the fetch left no copy, `InvalidPath` when a name is not inside the Workspace, `Layout`
/// when the reading does not resolve, and `Cancelled` when `cancellation` was set.
#[lazyffi(export = rola_write_ownership_plan)]
pub fn ownership_plan_at(
    request: &OwnershipInput,
    progress: &WriteProgress,
    cancellation: &Cancellation,
) -> Result<WriteOwnershipPlan, WriteError> {
    let request: OwnershipRequest = request.clone().into();
    let watched = crate::write::progress::watching(progress);
    let report = request.request_plan(&watched, cancellation)?;

    Ok(WriteOwnershipPlan::of(&report))
}

/// Moves the ownership a request names and a plan agreed to.
///
/// # FFI
///
/// `request` is the `RolaWriteOwnershipRequest` [`ownership_plan_at`] reads, with the same rule for
/// a relative `paths` entry. `wanted` and `wanted_len` are the caller's array of the entries the
/// plan promised to move, borrowed for the call, and a length of zero asks for the plan to be made
/// again and applied without comparing.
///
/// A `RolaWriteOwnershipApply *` the caller releases with `free_rola_write_ownership_apply`
/// answers it: `_entry_count` and `_entry_at` name each entry the Vault moved, with `owner` empty
/// when nobody holds it, and `_failed_count` and `_failed_at` name what was refused.
///
/// # Errors
///
/// Returns a `RolaWriteError *` the caller releases with `free_rola_write_error`, with what
/// [`ownership_plan_at`] names, and `Precondition` when `wanted` names entries this run would not
/// move.
#[lazyffi(export = rola_write_ownership_apply)]
pub fn ownership_apply_at(
    request: &OwnershipInput,
    wanted: &[OwnershipAgreed],
    progress: &WriteProgress,
    cancellation: &Cancellation,
) -> Result<WriteOwnershipApply, WriteError> {
    let request: OwnershipRequest = request.clone().into();
    let agreed: Vec<WantedView> = wanted.iter().cloned().map(WantedView::from).collect();
    let watched = crate::write::progress::watching(progress);
    let outcome = request.request_apply(
        (!agreed.is_empty()).then_some(agreed.as_slice()),
        &watched,
        cancellation,
    )?;

    Ok(WriteOwnershipApply::of(&outcome))
}

/// One entry an ownership plan means to move.
///
/// # FFI
///
/// A `RolaWriteOwnershipWanted *` the caller releases with `free_rola_write_ownership_wanted`.
/// `_path` and `_uuid` each return a `char *` the caller releases with `free_string`.
#[lazyffi(export = RolaWriteOwnershipWanted)]
#[derive(Debug, Clone, Default)]
pub struct WriteOwnershipWanted {
    /// Where the entry sits in the Layout.
    path: String,
    /// The `Uuid` ownership is about, as text.
    uuid: String,
}

impl WriteOwnershipWanted {
    /// One entry a plan names.
    fn of(wanted: &WantedView) -> Self {
        Self {
            path: wanted.path.clone(),
            uuid: wanted.uuid.clone(),
        }
    }
}

#[lazyffi(export = rola_)]
impl WriteOwnershipWanted {
    /// Where the entry sits in the Layout.
    #[must_use]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// The `Uuid` ownership is about, as text.
    #[must_use]
    pub fn uuid(&self) -> String {
        self.uuid.clone()
    }
}

/// Why an entry's ownership may not be moved, as the C ABI hands it over.
///
/// # FFI
///
/// A `RolaWriteOwnershipRefusal *` the caller releases with `free_rola_write_ownership_refusal`.
/// `_kind`, `_path`, `_owner` and `_cause` each return a `char *` the caller releases with
/// `free_string`; `_path` is empty when the refusal names no entry, `_owner` is empty when nobody
/// holds the entry, and `_cause` is empty unless the refusal carries a failure's words.
#[lazyffi(export = RolaWriteOwnershipRefusal)]
#[derive(Debug, Clone, Default)]
pub struct WriteOwnershipRefusal {
    /// The kind of refusal, as its word.
    kind: String,
    /// The path the refusal names, empty when it names none.
    path: String,
    /// The account that holds the entry, empty when it names none.
    owner: String,
    /// The failure's words, empty when the refusal carries none.
    cause: String,
}

impl WriteOwnershipRefusal {
    /// One refusal, as a C caller reads it.
    fn of(refusal: &Refusal) -> Self {
        let (kind, path, owner, cause) = match refusal {
            Refusal::Path(path) => ("path", path.clone(), String::new(), String::new()),
            Refusal::Upstream(path) => ("upstream", path.clone(), String::new(), String::new()),
            Refusal::Version(path) => ("version", path.clone(), String::new(), String::new()),
            Refusal::Changed(path) => ("changed", path.clone(), String::new(), String::new()),
            Refusal::Taken { path, owner } => ("taken", path.clone(), owner.clone(), String::new()),
            Refusal::NotHeld { path, owner } => (
                "not_held",
                path.clone(),
                owner.clone().unwrap_or_default(),
                String::new(),
            ),
            Refusal::Failed(cause) => ("failed", String::new(), String::new(), cause.clone()),
        };

        Self {
            kind: kind.to_owned(),
            path,
            owner,
            cause,
        }
    }
}

#[lazyffi(export = rola_)]
impl WriteOwnershipRefusal {
    /// The kind of refusal, as its word.
    #[must_use]
    pub fn kind(&self) -> String {
        self.kind.clone()
    }

    /// The path the refusal names, or an empty string when it names none.
    #[must_use]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// The account that holds the entry, or an empty string when it names none.
    #[must_use]
    pub fn owner(&self) -> String {
        self.owner.clone()
    }

    /// The failure's words, or an empty string when the refusal carries none.
    #[must_use]
    pub fn cause(&self) -> String {
        self.cause.clone()
    }
}

/// What an ownership plan would move, what it refused, and what it skipped.
///
/// # FFI
///
/// A `RolaWriteOwnershipPlan *` the caller releases with `free_rola_write_ownership_plan`.
/// `_wanted_count` and `_wanted_at` answer the entries a move would act on, each a
/// `RolaWriteOwnershipWanted *` released with `free_rola_write_ownership_wanted`; `_refused_count`
/// and `_refused_at` answer the refusals, each a `RolaWriteOwnershipRefusal *` released with
/// `free_rola_write_ownership_refusal`; and `_skipped` is how many files a directory brought in
/// that the Layout does not name.
#[lazyffi(export = RolaWriteOwnershipPlan)]
#[derive(Debug, Clone, Default)]
pub struct WriteOwnershipPlan {
    /// The entries a move would act on, in the plan's path order.
    wanted: Vec<WriteOwnershipWanted>,
    /// Why the gates refuse an entry, in path order.
    refused: Vec<WriteOwnershipRefusal>,
    /// How many files a directory brought in that the Layout does not name.
    skipped: usize,
}

impl WriteOwnershipPlan {
    /// The answer a plan is handed over as.
    fn of(report: &OwnershipPlanReport) -> Self {
        Self {
            wanted: report.wanted.iter().map(WriteOwnershipWanted::of).collect(),
            refused: report
                .refused
                .iter()
                .map(WriteOwnershipRefusal::of)
                .collect(),
            skipped: report.skipped,
        }
    }
}

#[lazyffi(export = rola_)]
impl WriteOwnershipPlan {
    /// How many entries a move would act on.
    #[must_use]
    pub const fn wanted_count(&self) -> usize {
        self.wanted.len()
    }

    /// One entry a move would act on, or null past the end.
    #[must_use]
    pub fn wanted_at(&self, index: usize) -> Option<WriteOwnershipWanted> {
        self.wanted.get(index).cloned()
    }

    /// How many entries the gates refuse.
    #[must_use]
    pub const fn refused_count(&self) -> usize {
        self.refused.len()
    }

    /// One refusal, or null past the end.
    #[must_use]
    pub fn refused_at(&self, index: usize) -> Option<WriteOwnershipRefusal> {
        self.refused.get(index).cloned()
    }

    /// How many files a directory brought in that the Layout does not name.
    #[must_use]
    pub const fn skipped(&self) -> usize {
        self.skipped
    }
}

/// One entry whose ownership the Vault agreed to move.
///
/// # FFI
///
/// A `RolaWriteOwnershipEntry *` the caller releases with `free_rola_write_ownership_entry`.
/// `_path`, `_uuid` and `_owner` each return a `char *` the caller releases with `free_string`;
/// `_owner` is empty when nobody holds the entry.
#[lazyffi(export = RolaWriteOwnershipEntry)]
#[derive(Debug, Clone, Default)]
pub struct WriteOwnershipEntry {
    /// The path the entry is named at.
    path: String,
    /// The entry's `Uuid`.
    uuid: String,
    /// The owner the Vault answered with, empty when nobody holds it.
    owner: String,
}

impl WriteOwnershipEntry {
    /// One moved entry, as a C caller reads it.
    fn of(item: &Item) -> Self {
        Self {
            path: item.path.clone(),
            uuid: item.uuid.clone(),
            owner: item.owner.clone().unwrap_or_default(),
        }
    }
}

#[lazyffi(export = rola_)]
impl WriteOwnershipEntry {
    /// The path the entry is named at.
    #[must_use]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    /// The entry's `Uuid`.
    #[must_use]
    pub fn uuid(&self) -> String {
        self.uuid.clone()
    }

    /// The owner the Vault answered with, or an empty string when nobody holds it.
    #[must_use]
    pub fn owner(&self) -> String {
        self.owner.clone()
    }
}

/// What an ownership move came to.
///
/// # FFI
///
/// A `RolaWriteOwnershipApply *` the caller releases with `free_rola_write_ownership_apply`.
/// `_entry_count` and `_entry_at` answer the entries the Vault moved, each a
/// `RolaWriteOwnershipEntry *` released with `free_rola_write_ownership_entry`; `_failed_count`
/// and `_failed_at` answer the refusals, each a `RolaWriteOwnershipRefusal *` released with
/// `free_rola_write_ownership_refusal`.
#[lazyffi(export = RolaWriteOwnershipApply)]
#[derive(Debug, Clone, Default)]
pub struct WriteOwnershipApply {
    /// The entries the Vault moved.
    entries: Vec<WriteOwnershipEntry>,
    /// What was refused.
    failed: Vec<WriteOwnershipRefusal>,
}

impl WriteOwnershipApply {
    /// The answer a move is handed over as.
    fn of(outcome: &Outcome) -> Self {
        Self {
            entries: outcome
                .entries
                .iter()
                .map(WriteOwnershipEntry::of)
                .collect(),
            failed: outcome
                .failed
                .iter()
                .map(WriteOwnershipRefusal::of)
                .collect(),
        }
    }
}

#[lazyffi(export = rola_)]
impl WriteOwnershipApply {
    /// How many entries the Vault moved.
    #[must_use]
    pub const fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// One entry the Vault moved, or null past the end.
    #[must_use]
    pub fn entry_at(&self, index: usize) -> Option<WriteOwnershipEntry> {
        self.entries.get(index).cloned()
    }

    /// How many entries were refused.
    #[must_use]
    pub const fn failed_count(&self) -> usize {
        self.failed.len()
    }

    /// One refusal, or null past the end.
    #[must_use]
    pub fn failed_at(&self, index: usize) -> Option<WriteOwnershipRefusal> {
        self.failed.get(index).cloned()
    }
}

/// Reads the version and, for a giveup, the tree.
fn settled(
    kind: Kind,
    workspace: &Workspace,
    path: &LayoutPath,
    layout: &Layout,
    upstream: &MutableData,
    diff: &TreeDiff,
    rules: &IgnoreRuleSet,
) -> Result<(), Refusal> {
    let Some(id) = layout.id_of(path) else {
        return Err(Refusal::Path(path.as_str().to_owned()));
    };

    if layout.entry(id).map(|data| data.version()) != Some(upstream.version()) {
        return Err(Refusal::Version(path.as_str().to_owned()));
    }

    if kind == Kind::Hold {
        return Ok(());
    }

    if diff.modified.contains(path) || tree_changed(workspace, layout, path, rules) {
        return Err(Refusal::Changed(path.as_str().to_owned()));
    }

    Ok(())
}

/// Whether a tree's work is not what the Layout records.
fn tree_changed(
    workspace: &Workspace,
    layout: &Layout,
    path: &LayoutPath,
    rules: &IgnoreRuleSet,
) -> bool {
    if !layout
        .id_of(path)
        .and_then(|id| layout.entry(id))
        .is_some_and(|data| data.kind() == EntryKind::Tree)
    {
        return false;
    }

    let root = workspace.get_root();
    let disk = root.join(path.to_path_buf());
    let Ok(tree) = Tree::open(root, &disk) else {
        return true;
    };

    if tree.pending().unwrap_or(true) {
        return true;
    }

    let Ok(Some(recorded)) = tree.baseline().tree() else {
        return false;
    };
    let Ok(rules) = tree.rules(rules.clone()) else {
        return true;
    };
    let Ok(scanned) = tree.scan(&rules, false) else {
        return true;
    };

    !crate::tree::diff(&recorded, &scanned.structure).is_empty()
}

/// Reads who holds it, which is the gate force does not move.
fn free(kind: Kind, path: &LayoutPath, upstream: &MutableData, me: &str) -> Result<(), Refusal> {
    let named = path.as_str().to_owned();

    match (kind, upstream.owner()) {
        (Kind::Hold, None) => Ok(()),
        (Kind::Hold, Some(owner)) => Err(Refusal::Taken {
            path: named,
            owner: owner.to_owned(),
        }),
        (Kind::Giveup, Some(owner)) if owner == me => Ok(()),
        (Kind::Giveup, owner) => Err(Refusal::NotHeld {
            path: named,
            owner: owner.map(str::to_owned),
        }),
    }
}

/// The path `name` names, made absolute against the directory the run was made in.
fn resolve(cwd: &Path, name: &str) -> PathBuf {
    if Path::new(name).is_absolute() {
        PathBuf::from(name)
    } else {
        cwd.join(name)
    }
}

/// Whether `path` sits under the directory whose prefix is `prefix`.
fn under(prefix: &str, path: &LayoutPath) -> bool {
    if prefix.is_empty() {
        return true;
    }

    path.as_str()
        .strip_prefix(prefix)
        .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_layout::LayoutPath;
    use rorolala_tree::Staging;
    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;
    use uuid::Uuid;

    use super::{Wanted, WantedView, expand, same_entries};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn scratch(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rola-write-ownership-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");

        dir
    }

    /// Naming a tree or anything under it names the tree's one Layout entry.
    #[test]
    fn a_tree_and_everything_under_it_names_the_tree() {
        let root = scratch("tree");
        Workspace::create(&root).expect("a Workspace");
        let workspace = Workspace::locate(&root).expect("the Workspace is there");
        let tree = root.join("main");
        fs::create_dir_all(tree.join("sub")).expect("a tree's directory");
        Staging::new(&tree).ensure_store().expect("a tree's mark");
        fs::write(root.join("beside.psd"), b"x").expect("a file beside the tree");

        let named = |name: &str| {
            expand(&workspace, &root, &[name.to_owned()])
                .expect("the name reads")
                .into_iter()
                .map(|named| (named.path.as_str().to_owned(), named.direct))
                .collect::<Vec<_>>()
        };

        assert_eq!(named("main"), vec![("main".to_owned(), true)]);
        assert_eq!(named("main/sub"), vec![("main".to_owned(), true)]);
        assert_eq!(named("beside.psd"), vec![("beside.psd".to_owned(), true)]);

        let _ = fs::remove_dir_all(&root);
    }

    /// A plan names entries rather than text: the order and the `Uuid` case are not part of what it
    /// agrees to, while an entry it does not name is not the plan.
    #[test]
    fn a_plan_is_the_entries_it_names() {
        let first = Uuid::from_u128(7);
        let second = Uuid::from_u128(8);
        let wanted = vec![
            Wanted {
                path: LayoutPath::new("a/b.psd").expect("a path"),
                id: first,
            },
            Wanted {
                path: LayoutPath::new("c/d.psd").expect("a path"),
                id: second,
            },
        ];
        let view = |path: &str, uuid: &str| WantedView {
            path: path.to_owned(),
            uuid: uuid.to_owned(),
        };

        assert!(same_entries(
            &wanted,
            &[
                view("c/d.psd", &second.to_string()),
                view("a/b.psd", &first.to_string()),
            ]
        ));
        assert!(same_entries(
            &wanted,
            &[
                view("c/d.psd", &second.to_string().to_uppercase()),
                view("a/b.psd", &first.to_string().to_uppercase()),
            ]
        ));
        assert!(!same_entries(
            &wanted,
            &[
                view("c/d.psd", &second.to_string()),
                view("a/b.psd", &Uuid::from_u128(9).to_string()),
            ]
        ));
        assert!(!same_entries(
            &wanted,
            &[view("c/d.psd", &second.to_string())]
        ));
        assert!(!same_entries(&wanted, &[]));
    }
}
