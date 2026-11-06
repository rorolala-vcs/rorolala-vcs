//! The `rola tree desc` command: write the description of the next tree version.

// The handler's parameters are the resources the framework injects, so how many of them there are is
// what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_arguments, clippy::trivially_copy_pass_by_ref)]

use std::fs;

use librorolala::tree::Staging;
use mingling::{
    Grouped, LazyRes, Suggest,
    consts::UNMATCHED,
    macros::{arg, buffer, command, completion, help, metadata, r_eprintln, renderer},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResWorkspace;
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rorolala_utils_constants::WORKSPACE_EDITING_PATH;
use rorolala_utils_location::Locate as _;
use rust_i18n::t;

use crate::Next;
use crate::editor::{ResEditor, open};
use crate::exit_codes::{EC_ERR_TREE_FAILED, EC_HELP};
use crate::failure::failure;
use crate::tree::cmd_tree::{ErrorTreeShouldInWorkspace, locate};

#[help(buffer)]
pub fn help_tree_desc(_: EntryTreeDesc, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tree.desc.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryTreeDesc)]
pub fn desc_tree_desc() -> Description {
    t!("tree.desc.description").to_string().into()
}

/// Completes what `rola tree desc` can be given next.
///
/// The tree is the filesystem's to answer, and there is nothing else the command takes.
#[completion(EntryTreeDesc)]
pub fn complete_tree_desc() -> Suggest {
    Suggest::file_comp()
}

/// Writes the description of the next version of a tree
///
/// A description is the body of a version rather than its message: `rola track` asks for what was
/// done, and this is the longer text that can be read back with
/// [`rola tree cat-desc`](crate::tree::cmd_tree_cat_desc). It is optional, and an empty file clears
/// it.
///
/// `TREE` names the tree: its root, a path inside it, or `.` for where the run was made. The editor
/// is the run's own, the same one `rola track` opens, and it works on the same editing file: what it
/// wrote is staged only once it ends successfully, so a cancelled run leaves the description as it
/// was.
///
/// # Errors
///
/// Renders the editor's failure when it cannot be opened or does not end with the text kept,
/// [`ErrorTreeShouldInWorkspace`] when the run is not inside a Workspace, and
/// [`ErrorTreeDescFailed`] when the tree cannot be found.
#[command(node = "tree.desc", entry = EntryTreeDesc)]
pub fn tree_desc(
    args: EntryTreeDesc,
    workspace: &mut LazyRes<ResWorkspace>,
    editor: &ResEditor,
) -> Next {
    let picked = args.pick(arg![Option<String>]).pick(&UNMATCHED).to_result();
    let (target, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "tree desc") {
        return next;
    }

    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorTreeShouldInWorkspace.into();
    };

    let tree = match locate(held, target.as_deref()) {
        Ok(tree) => tree,
        Err(next) => return next,
    };

    let staging = Staging::new(tree.root());
    if let Err(error) = staging.ensure_store() {
        return ErrorTreeDescFailed::new(error.to_string()).into();
    }

    let existing = match staging.description() {
        Ok(description) => description.unwrap_or_default(),
        Err(error) => return ErrorTreeDescFailed::new(error.to_string()).into(),
    };

    let editor = match editor.must_bind() {
        Ok(editor) => editor,
        Err(error) => return error.into(),
    };

    // The editor works on a scratch file, as `rola track` does, and what it wrote is staged only
    // after it ends well: an editor that fails, or a run stopped while it is open, leaves the
    // description that was staged before untouched.
    let path = held.get_root().join(WORKSPACE_EDITING_PATH);
    let header = editing_header();
    if let Err(error) = fs::write(&path, format!("{header}{existing}")) {
        return ErrorTreeDescFailed::new(error.to_string()).into();
    }

    if let Err(error) = open(editor, &path) {
        return error.into();
    }

    let edited = match fs::read_to_string(&path) {
        Ok(edited) => edited,
        Err(error) => return ErrorTreeDescFailed::new(error.to_string()).into(),
    };

    if let Err(error) = staging.write_description(description_of(&edited, &header).as_deref()) {
        return ErrorTreeDescFailed::new(error.to_string()).into();
    }

    ResultTreeDesc.into()
}

/// Result: the description was written.
#[derive(Grouped)]
pub struct ResultTreeDesc;

#[renderer(buffer)]
pub fn render_result_tree_desc(_result: ResultTreeDesc, exit: &mut ResExitCode) {
    let _ = exit;
}

/// What the editing file says above the description it holds.
fn editing_header() -> String {
    format!("# {}\n\n", t!("tree.desc.editing_header").trim())
}

/// The description a saved editing file holds, or nothing when it clears one.
///
/// The note the file opens with is not the description: the text below it is. An editor that kept
/// the note but says nothing below it is a run clearing the description, and one that is left as it
/// was opens the description that was already staged.
fn description_of(text: &str, header: &str) -> Option<String> {
    let text = text.replace("\r\n", "\n");
    let body = text.strip_prefix(header).map_or_else(
        || {
            // A run that deleted or rewrote the note still says what it wants: what is left is read
            // as the description, with its note above it dropped the way a comment line is.
            let mut body = Vec::new();
            let mut skipping = true;

            for line in text.lines() {
                if skipping && (line.trim().is_empty() || line.trim_start().starts_with('#')) {
                    continue;
                }

                skipping = false;
                body.push(line);
            }

            body.join("\n")
        },
        str::to_owned,
    );

    let body = body.trim().to_owned();

    (!body.is_empty()).then_some(body)
}

/// Error: the tree cannot be read.
#[derive(Grouped)]
pub struct ErrorTreeDescFailed {
    /// What happened.
    cause: String,
}

impl ErrorTreeDescFailed {
    /// The failure of writing a description, for `cause`.
    fn new(cause: impl Into<String>) -> Self {
        Self {
            cause: cause.into(),
        }
    }
}

impl Failure for ErrorTreeDescFailed {
    fn name(&self) -> &'static str {
        "error_tree_desc_failed"
    }

    fn reason(&self) -> String {
        t!("tree.desc.err_failed", cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorTreeDescFailed);

#[renderer(buffer)]
pub fn render_error_tree_desc_failed(error: ErrorTreeDescFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("tree.desc.err_failed_help").trim()));
    ec.exit_code = EC_ERR_TREE_FAILED;
}

#[cfg(test)]
mod tests {
    use super::description_of;

    /// The note the file opens with is not the description, and an empty body clears one.
    #[test]
    fn the_note_is_not_the_description() {
        let header = "# write below this line\n\n";

        assert_eq!(
            description_of("# write below this line\n\nbody\n", header).as_deref(),
            Some("body")
        );
        assert_eq!(description_of("# write below this line\n\n", header), None);
        assert_eq!(description_of("", header), None);
        assert_eq!(
            description_of("# write below this line\r\n\r\nold\r\n", header).as_deref(),
            Some("old")
        );
    }

    /// A description keeps its own leading comment lines when the note was torn away or rewritten.
    #[test]
    fn a_rewritten_note_leaves_what_is_below_it() {
        let header = "# write below this line\n\n";

        assert_eq!(
            description_of("# mine\nbody\n", header).as_deref(),
            Some("body")
        );
    }
}
