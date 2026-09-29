//! The `rola layout` namespace: the Layouts a Workspace works in and a Vault keeps.
//!
//! A Workspace holds many Layouts and works in one at a time; a Vault holds one. What is here is
//! what the two have in common — where a run's Layouts are, and how a failure to work on them is
//! said — beside the subcommands, each in a file of its own: listing them, and, under those, making,
//! copying, tracking, exporting, importing and removing them.

pub mod cmd_layout;
pub mod cmd_layout_cp;
pub mod cmd_layout_entries;
pub mod cmd_layout_entry;
pub mod cmd_layout_force_switch;
pub mod cmd_layout_import;
pub mod cmd_layout_ls;
pub mod cmd_layout_new;
pub mod cmd_layout_path;
pub mod cmd_layout_rm;
pub mod cmd_layout_set_track;
pub mod cmd_layout_shot;
pub mod cmd_layout_tree_diff;
pub mod cmd_layout_un_track;

use mingling::{
    Grouped,
    macros::{buffer, r_eprintln, r_println, renderer},
    picker::Pickable,
    res::ResExitCode,
};
use rorolala_cli_setups::{ResVault, ResWorkspace};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line};
use rust_i18n::t;

use librorolala::layout::{Layout, LayoutError, Layouts};

use crate::Next;
use crate::exit_codes::{
    EC_ALREADY_EXIST, EC_ERR_LAYOUT, EC_ERR_LAYOUT_ARGUMENT, EC_ERR_SHOULD_IN_WORKSPACE,
    EC_NOT_EXIST,
};
use crate::failure::failure;

/// What a run works its Layouts through.
pub enum Place {
    /// A Workspace, with its set of named Layouts.
    Workspace(Layouts),

    /// A Vault, with its one Layout.
    Vault(Layout),
}

/// The place a run works its Layouts in, from what the run found.
///
/// A Workspace comes before a Vault: a run inside one works in its Layouts, whatever else the
/// directories above it hold. A Vault keeps its one Layout directly, so it is opened here.
///
/// # Errors
///
/// Answers with [`ErrorNoLayouts`] when the run is in neither, and [`ErrorLayoutFailed`] when a
/// Vault's Layout could not be opened.
pub fn place(workspace: &ResWorkspace, vault: &ResVault) -> Result<Place, Next> {
    if let Some(workspace) = workspace.as_ref() {
        return Ok(Place::Workspace(workspace.layouts()));
    }

    let Some(vault) = vault.as_ref() else {
        return Err(ErrorNoLayouts.into());
    };

    vault.layout().map(Place::Vault).map_err(|error| {
        ErrorLayoutFailed {
            cause: error.to_string(),
        }
        .into()
    })
}

/// Error: this run is in neither a Workspace nor a Vault, so it has no Layouts to work on.
#[derive(Grouped)]
pub struct ErrorNoLayouts;

impl Failure for ErrorNoLayouts {
    fn name(&self) -> &'static str {
        "error_no_layouts"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_no_layouts").trim().to_string()
    }
}

failure!(ErrorNoLayouts);

#[renderer(buffer)]
pub fn render_error_no_layouts(_: ErrorNoLayouts, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("cmd_layout.err_no_layouts").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_no_layouts_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT;
}

/// Error: a Layout could not be worked on.
///
/// The reason is the library's own words, since the Layout says what went wrong about it — a path
/// already taken, a name that is not one, or bytes that would not be read.
#[derive(Grouped)]
pub struct ErrorLayoutFailed {
    /// Why the Layout would not be worked on.
    cause: String,
}

impl ErrorLayoutFailed {
    /// The failure of a Layout that could not be worked on, saying why in `cause`.
    #[must_use]
    pub fn new(cause: impl Into<String>) -> Self {
        Self {
            cause: cause.into(),
        }
    }
}

impl Failure for ErrorLayoutFailed {
    fn name(&self) -> &'static str {
        "error_layout_failed"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_layout_failed", reason = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorLayoutFailed);

#[renderer(buffer)]
pub fn render_error_layout_failed(error: ErrorLayoutFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_layout_failed_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT;
}

/// The Layout a run works on: the one named, or the one being worked in.
///
/// What is named is a Workspace's to choose; a Vault keeps one Layout and has nothing to name.
///
/// # Errors
///
/// Answers with the rendered failures of [`place`], with [`ErrorLayoutArgument`] when nothing is
/// named and nothing is being worked in, or when a Vault run named one, and with
/// [`ErrorLayoutMissing`] when the one named is not there.
pub fn chosen(
    workspace: &ResWorkspace,
    vault: &ResVault,
    named: Option<&str>,
) -> Result<Layout, Next> {
    match place(workspace, vault)? {
        Place::Workspace(layouts) => {
            let name = match named {
                Some(name) => name.to_owned(),
                None => match layouts.current() {
                    Ok(Some(name)) => name,
                    Ok(None) => return Err(ErrorLayoutArgument.into()),
                    Err(error) => return Err(failed(&error)),
                },
            };

            match layouts.get(&name) {
                Ok(Some(layout)) => Ok(layout),
                Ok(None) => Err(ErrorLayoutMissing.into()),
                Err(error) => Err(failed(&error)),
            }
        }
        Place::Vault(layout) => {
            if named.is_some() {
                return Err(ErrorLayoutArgument.into());
            }

            Ok(layout)
        }
    }
}

/// Error: a Layout command works on a Workspace, and this run is not inside one.
///
/// It is the placement failure already said elsewhere, under a name this namespace can return: what
/// a person is shown is the same sentence, since the two mean the same thing.
#[derive(Grouped)]
pub struct ErrorLayoutShouldInWorkspace;

impl Failure for ErrorLayoutShouldInWorkspace {
    fn name(&self) -> &'static str {
        "error_layout_should_in_workspace"
    }

    fn reason(&self) -> String {
        t!("error.placement.err_should_in_workspace")
            .trim()
            .to_string()
    }
}

failure!(ErrorLayoutShouldInWorkspace);

#[renderer(buffer)]
pub fn render_error_layout_should_in_workspace(
    _: ErrorLayoutShouldInWorkspace,
    ec: &mut ResExitCode,
) {
    r_eprintln!(
        "{}",
        err_line!(t!("error.placement.err_should_in_workspace").trim())
    );
    r_eprintln!(
        "{}",
        help_line!(t!("error.placement.err_should_in_workspace_help").trim())
    );
    ec.exit_code = EC_ERR_SHOULD_IN_WORKSPACE;
}

/// Error: a `rola layout` command was given arguments it cannot use.
#[derive(Grouped)]
pub struct ErrorLayoutArgument;

impl Failure for ErrorLayoutArgument {
    fn name(&self) -> &'static str {
        "error_layout_argument"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_layout_argument").trim().to_string()
    }
}

failure!(ErrorLayoutArgument);

#[renderer(buffer)]
pub fn render_error_layout_argument(_: ErrorLayoutArgument, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("cmd_layout.err_layout_argument").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_layout_argument_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT_ARGUMENT;
}

/// Error: there is already a Layout by that name.
#[derive(Grouped)]
pub struct ErrorLayoutExists;

impl Failure for ErrorLayoutExists {
    fn name(&self) -> &'static str {
        "error_layout_exists"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_layout_exists").trim().to_string()
    }
}

failure!(ErrorLayoutExists);

#[renderer(buffer)]
pub fn render_error_layout_exists(_: ErrorLayoutExists, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("cmd_layout.err_layout_exists").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_layout_exists_help").trim())
    );
    ec.exit_code = EC_ALREADY_EXIST;
}

/// Error: there is no Layout by that name.
#[derive(Grouped)]
pub struct ErrorLayoutMissing;

impl Failure for ErrorLayoutMissing {
    fn name(&self) -> &'static str {
        "error_layout_missing"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_layout_missing").trim().to_string()
    }
}

failure!(ErrorLayoutMissing);

#[renderer(buffer)]
pub fn render_error_layout_missing(_: ErrorLayoutMissing, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(t!("cmd_layout.err_layout_missing").trim()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_layout_missing_help").trim())
    );
    ec.exit_code = EC_NOT_EXIST;
}

/// Error: the Vault a Layout was to track is not one the Workspace has bound.
#[derive(Grouped)]
pub struct ErrorLayoutTrackNotBound {
    /// The name that is not bound.
    pub track: String,
}

impl Failure for ErrorLayoutTrackNotBound {
    fn name(&self) -> &'static str {
        "error_layout_track_not_bound"
    }

    fn reason(&self) -> String {
        t!("cmd_layout.err_track_not_bound", track = self.track)
            .trim()
            .to_string()
    }
}

failure!(ErrorLayoutTrackNotBound);

#[renderer(buffer)]
pub fn render_error_layout_track_not_bound(error: ErrorLayoutTrackNotBound, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("cmd_layout.err_track_not_bound_help").trim())
    );
    ec.exit_code = EC_ERR_LAYOUT_ARGUMENT;
}

/// What a Layout that could not be worked on is answered with.
///
/// A name already taken and a name with nothing behind it are told apart, since what is to be done
/// about them differs; everything else is the library's own words, under the one exit code.
pub fn failed(error: &LayoutError) -> Next {
    match error {
        LayoutError::AlreadyExists => ErrorLayoutExists.into(),
        LayoutError::NotFound => ErrorLayoutMissing.into(),
        other => ErrorLayoutFailed {
            cause: other.to_string(),
        }
        .into(),
    }
}

/// The flags a command that works on one Layout takes.
#[derive(Pickable)]
pub struct LayoutOnlyFlags {
    /// The Layout to work on; the one being worked in when none is named.
    #[arg(long)]
    pub layout: Option<String>,
}

/// What a Layout-content command did, for the one sentence that says so.
#[derive(Grouped, Clone, Copy)]
pub enum LayoutDid {
    /// An entry was made.
    Created,
    /// An entry's data was changed.
    Updated,
    /// An entry was dropped.
    Removed,
    /// A path was bound.
    Bound,
    /// A path was unbound.
    Unbound,
    /// A path was moved.
    Moved,
}

/// Result: a Layout's content was changed.
#[derive(Grouped)]
pub struct ResultLayoutContent {
    /// What was done.
    pub did: LayoutDid,
    /// What it was done to.
    pub what: String,
}

#[renderer(buffer)]
pub fn render_result_layout_content(result: ResultLayoutContent) {
    let said = match result.did {
        LayoutDid::Created => t!("cmd_layout_entry.result_created", what = result.what),
        LayoutDid::Updated => t!("cmd_layout_entry.result_updated", what = result.what),
        LayoutDid::Removed => t!("cmd_layout_entry.result_removed", what = result.what),
        LayoutDid::Bound => t!("cmd_layout_path.result_bound", what = result.what),
        LayoutDid::Unbound => t!("cmd_layout_path.result_unbound", what = result.what),
        LayoutDid::Moved => t!("cmd_layout_path.result_moved", what = result.what),
    };

    r_println!("{}", said.trim());
}
