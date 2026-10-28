//! The ignore rules a run reads the Workspace's tree by.
//!
//! The rules themselves are the library's — see [`librorolala::ignore`] — and what lives here is
//! the one flag every command that honours them takes, and the loading they all share: a run that
//! was given `--include-ignored` reads with the empty set, which is the whole of what the flag
//! asks for.

use librorolala::ignore::IgnoreRuleSet;
use librorolala::workspace::Workspace;
use mingling::macros::arg;
use mingling::picker::{PickerArg, value::Flag};
use rorolala_utils_cli_theme::warn_line;
use rorolala_utils_constants::WORKSPACE_IGNORE_RULES_DIR;
use rorolala_utils_location::Locate as _;
use rust_i18n::t;

/// Include what the ignore rules govern, rather than leaving it out or refusing it.
pub const ARG_INCLUDE_IGNORED: PickerArg<'static, Flag> = arg![include_ignored: Flag];

/// The rules `held` ignores by, or the empty set when the run was asked to include the ignored.
///
/// A set that cannot be read is answered with the empty one and a word on stderr: the reading the
/// run set out to make is the thing being answered, and the rules are a refinement of it — but a
/// refinement that failed is worth a mention, since its absence hides nothing.
#[must_use]
pub fn rules(held: &Workspace, include_ignored: bool) -> IgnoreRuleSet {
    if include_ignored {
        return IgnoreRuleSet::empty();
    }

    IgnoreRuleSet::load(
        &held.get_root().join(WORKSPACE_IGNORE_RULES_DIR),
        held.get_root(),
    )
    .unwrap_or_else(|error| {
        eprintln!(
            "{}",
            warn_line!(t!("ignore.err_rules", reason = error.to_string()).trim())
        );

        IgnoreRuleSet::empty()
    })
}

/// Whether `path` may be selected by a run reading with `rules`: not ignored, or a move's
/// destination, which a rule is not written against.
///
/// The destinations are the tree reading's own: a file that turned out to be where a tracked path
/// went is already the work's, however the rules read its new place.
#[must_use]
pub fn selectable(
    rules: &IgnoreRuleSet,
    path: &librorolala::layout::LayoutPath,
    renames: &[librorolala::tree_analyze::PathRename],
) -> bool {
    if renames.iter().any(|rename| rename.to == *path) {
        return true;
    }

    !rules.is_ignore(std::path::Path::new(path.as_str()), false)
}
