//! The `rola tree write-desc` command: write the description of the next tree version directly.

// The handler's parameters are the resources the framework injects, so how many of them there are is
// what the command needs of the run rather than a signature this program shaped.
#![allow(clippy::too_many_arguments, clippy::trivially_copy_pass_by_ref)]

use std::io::Read as _;

use librorolala::tree::Staging;
use mingling::{
    Grouped, LazyRes, Suggest,
    consts::UNMATCHED,
    macros::{arg, buffer, command, completion, help, metadata, r_eprintln, r_println, renderer},
    metadata::Description,
    picker::EntryPicker,
    res::ResExitCode,
};
use rorolala_cli_setups::ResWorkspace;
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::{EC_ERR_TREE_FAILED, EC_HELP};
use crate::failure::failure;
use crate::tree::cmd_tree::{ErrorTreeShouldInWorkspace, locate};

#[help(buffer)]
pub fn help_tree_write_desc(_: EntryTreeWriteDesc, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("tree.write_desc.help")).trim());
    ec.exit_code = EC_HELP;
}

#[metadata(EntryTreeWriteDesc)]
pub fn desc_tree_write_desc() -> Description {
    t!("tree.write_desc.description").to_string().into()
}

/// Completes what `rola tree write-desc` can be given next.
///
/// The text is the caller's own words, so there is nothing to list.
#[completion(EntryTreeWriteDesc)]
pub fn complete_tree_write_desc() -> Suggest {
    Suggest::file_comp()
}

/// Writes the description of the next version of a tree without an editor
///
/// `TEXT` is the description. With no text, or with `-`, it is read from stdin, which is how a long
/// description is written from a file. An empty description clears it.
///
/// # Errors
///
/// Renders [`ErrorTreeWriteDescArgument`] when stdin cannot be read,
/// [`ErrorTreeShouldInWorkspace`] when the run is not inside a Workspace, and
/// [`ErrorTreeWriteDescFailed`] when the description cannot be written.
#[command(node = "tree.write-desc", entry = EntryTreeWriteDesc)]
pub fn tree_write_desc(args: EntryTreeWriteDesc, workspace: &mut LazyRes<ResWorkspace>) -> Next {
    let picked = args.pick(arg![Option<String>]).pick(&UNMATCHED).to_result();
    let (text, unmatched) = match picked {
        Ok(picked) => picked,
        Err(next) => return next,
    };

    if let Some(next) = crate::unmatched::refused(&unmatched, "tree write-desc") {
        return next;
    }

    let Some(held) = workspace.get_ref().as_ref() else {
        return ErrorTreeShouldInWorkspace.into();
    };

    let tree = match locate(held, None) {
        Ok(tree) => tree,
        Err(next) => return next,
    };

    let text = match text.as_deref() {
        Some("-") | None => {
            let mut read = String::new();
            if let Err(error) = std::io::stdin().read_to_string(&mut read) {
                return ErrorTreeWriteDescArgument::new(error.to_string()).into();
            }

            read
        }
        Some(text) => text.to_owned(),
    };

    let staging = Staging::new(tree.root());
    if let Err(error) = staging.ensure_store() {
        return ErrorTreeWriteDescFailed::new(error.to_string()).into();
    }
    if let Err(error) = staging.write_description(Some(&text)) {
        return ErrorTreeWriteDescFailed::new(error.to_string()).into();
    }

    ResultTreeWriteDesc { bytes: text.len() }.into()
}

/// Result: the description was written.
#[derive(Grouped)]
pub struct ResultTreeWriteDesc {
    /// How many bytes the description is.
    bytes: usize,
}

#[renderer(buffer)]
pub fn render_result_tree_write_desc(result: ResultTreeWriteDesc, ec: &mut ResExitCode) {
    if result.bytes == 0 {
        r_println!("{}", t!("tree.write_desc.cleared").trim());
    } else {
        r_println!(
            "{}",
            t!("tree.write_desc.written", bytes = result.bytes.to_string()).trim()
        );
    }

    let _ = ec;
}

/// Error: the text cannot be read.
#[derive(Grouped)]
pub struct ErrorTreeWriteDescArgument {
    /// What happened.
    cause: String,
}

impl ErrorTreeWriteDescArgument {
    /// The failure of reading a description, for `cause`.
    fn new(cause: impl Into<String>) -> Self {
        Self {
            cause: cause.into(),
        }
    }
}

impl Failure for ErrorTreeWriteDescArgument {
    fn name(&self) -> &'static str {
        "error_tree_write_desc_argument"
    }

    fn reason(&self) -> String {
        t!("tree.write_desc.err_argument", cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorTreeWriteDescArgument);

#[renderer(buffer)]
pub fn render_error_tree_write_desc_argument(
    error: ErrorTreeWriteDescArgument,
    ec: &mut ResExitCode,
) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("tree.write_desc.err_argument_help").trim())
    );
    ec.exit_code = EC_ERR_TREE_FAILED;
}

/// Error: the description cannot be written.
#[derive(Grouped)]
pub struct ErrorTreeWriteDescFailed {
    /// What happened.
    cause: String,
}

impl ErrorTreeWriteDescFailed {
    /// The failure of writing a description, for `cause`.
    fn new(cause: impl Into<String>) -> Self {
        Self {
            cause: cause.into(),
        }
    }
}

impl Failure for ErrorTreeWriteDescFailed {
    fn name(&self) -> &'static str {
        "error_tree_write_desc_failed"
    }

    fn reason(&self) -> String {
        t!("tree.write_desc.err_failed", cause = self.cause)
            .trim()
            .to_string()
    }
}

failure!(ErrorTreeWriteDescFailed);

#[renderer(buffer)]
pub fn render_error_tree_write_desc_failed(error: ErrorTreeWriteDescFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!(
        "{}",
        help_line!(t!("tree.write_desc.err_failed_help").trim())
    );
    ec.exit_code = EC_ERR_TREE_FAILED;
}
