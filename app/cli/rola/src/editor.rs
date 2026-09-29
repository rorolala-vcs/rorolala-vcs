//! The editor a run opens to write what it is recording.
//!
//! Recording is not always something a caller can say in one argument: a message covers a group of
//! files and each file may want its own words about itself, and a run that was told neither has to
//! be asked. What is asked is written down as a file and opened in an editor — the same way the
//! language and the account are read, from a global argument and then the environment — so what a
//! run asks for is a program to open, and everything after that is the caller's.

use std::env;
use std::path::Path;
use std::process::Command;

use mingling::{
    Grouped, ProgramCollect,
    macros::{arg, buffer, r_eprintln, renderer},
    picker::{PickerArg, PickerHelper},
    res::ResExitCode,
    setup::ProgramSetup,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{err_line, help_line};
use rust_i18n::t;

use crate::exit_codes::EC_ERR_EDITOR;
use crate::failure::failure;

/// Environment variables consulted, in order, when `--editor` names none
///
/// The first one set wins. `ROLA_EDITOR` is the project's own; `EDITOR` is the convention a shell
/// and a terminal already carry.
const EDITOR_ENV_VARS: &[&str] = &["ROLA_EDITOR", "EDITOR"];

/// Global argument naming the editor a run opens
///
/// Usage: `--editor="code --wait"`, or `--editor=""` to say no editor is to be opened.
pub const GLOBAL_ARG_EDITOR: PickerArg<'static, String> = arg![editor: String];

/// The editor a run opens, as the run named one.
///
/// An editor named as an empty string is a run saying it has none: the distinction matters because
/// a run that named none at all falls back to the environment, while one that named emptiness is
/// asking to be refused when a command has to open something.
#[derive(Debug, Default, Clone)]
pub struct ResEditor {
    /// The editor to open, or nothing when the run named none or named emptiness.
    editor: Option<String>,
    /// Whether `--editor` was given at all, which is what tells naming none from naming emptiness.
    named: bool,
}

impl ResEditor {
    /// The editor to open, or why there is none to open.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorNoEditor`] when neither the run nor the environment named one.
    pub fn must_bind(&self) -> Result<&str, ErrorNoEditor> {
        self.editor
            .as_deref()
            .ok_or(ErrorNoEditor { named: self.named })
    }
}

/// Error: a run has to open an editor, and none was named it can open.
///
/// A run that named `--editor=""` is told apart from one that named nothing: the first said it has
/// none, the second never got one, and what to do about them differs.
#[derive(Grouped)]
pub struct ErrorNoEditor {
    /// Whether the run named an editor at all.
    named: bool,
}

impl Failure for ErrorNoEditor {
    fn name(&self) -> &'static str {
        "error_no_editor"
    }

    fn reason(&self) -> String {
        let said = if self.named {
            t!("editor.err_named_none")
        } else {
            t!("editor.err_no_editor")
        };

        said.trim().to_string()
    }
}

failure!(ErrorNoEditor);

#[renderer(buffer)]
pub fn render_error_no_editor(error: ErrorNoEditor, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("editor.err_no_editor_help").trim()));
    ec.exit_code = EC_ERR_EDITOR;
}

/// Error: the editor could not be opened, or ended without the message being kept.
#[derive(Grouped)]
pub struct ErrorEditorFailed {
    /// The editor that was opened.
    editor: String,
    /// Why it did not end as it had to.
    cause: String,
}

impl Failure for ErrorEditorFailed {
    fn name(&self) -> &'static str {
        "error_editor_failed"
    }

    fn reason(&self) -> String {
        t!(
            "editor.err_editor_failed",
            editor = self.editor,
            reason = self.cause
        )
        .trim()
        .to_string()
    }
}

failure!(ErrorEditorFailed);

#[renderer(buffer)]
pub fn render_error_editor_failed(error: ErrorEditorFailed, ec: &mut ResExitCode) {
    r_eprintln!("{}", err_line!(error.reason()));
    r_eprintln!("{}", help_line!(t!("editor.err_editor_failed_help").trim()));
    ec.exit_code = EC_ERR_EDITOR;
}

/// A [`ProgramSetup`] that turns `--editor` and the environment into [`ResEditor`].
///
/// `--editor` comes first — including when it names emptiness, which is a run saying it has none —
/// and the environment is consulted only when the argument is absent, so a run that names an editor
/// is never overridden by one the shell happens to carry.
pub struct EditorSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for EditorSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        let asked = program.pick_argument(&GLOBAL_ARG_EDITOR);
        let named = asked.is_some();

        let editor = asked.map_or_else(
            || {
                EDITOR_ENV_VARS
                    .iter()
                    .find_map(|name| env::var(name).ok())
                    .filter(|editor| !editor.is_empty())
            },
            |editor| Some(editor).filter(|editor| !editor.is_empty()),
        );

        program.with_resource(ResEditor { editor, named });
    }
}

/// Opens `editor` on `path`, waiting for it to end.
///
/// The editor is named the way a shell would write one — a program and any arguments of its own —
/// so what is split is words, and `path` is handed on after them. An editor that ends having failed,
/// or that cannot be started at all, is a run that did not get what it asked for.
///
/// # Errors
///
/// Returns [`ErrorEditorFailed`] when the editor names no program, cannot be started, or ends
/// without success.
pub fn open(editor: &str, path: &Path) -> Result<(), ErrorEditorFailed> {
    let mut words = editor.split_whitespace();
    let Some(program) = words.next() else {
        return Err(ErrorEditorFailed {
            editor: editor.to_owned(),
            cause: t!("editor.err_empty_program").trim().to_string(),
        });
    };

    let status = Command::new(program).args(words).arg(path).status();

    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(ErrorEditorFailed {
            editor: editor.to_owned(),
            cause: t!("editor.err_editor_status", status = status.to_string())
                .trim()
                .to_string(),
        }),
        Err(error) => Err(ErrorEditorFailed {
            editor: editor.to_owned(),
            cause: error.to_string(),
        }),
    }
}
