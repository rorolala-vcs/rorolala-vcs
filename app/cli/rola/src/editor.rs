//! The editor a run opens to write what it is recording.
//!
//! Recording is not always something a caller can say in one argument: a message covers a group of
//! files and each file may want its own words about itself, and a run that was told neither has to
//! be asked. What is asked is written down as a file and opened in an editor — the same way the
//! language and the account are read, from a global argument and then the environment, and at the
//! last from the `rola-edit` beside `rola` — so what a run asks for is a program to open, and
//! everything after that is the caller's.

use std::env;
use std::path::{Path, PathBuf};
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

/// The editor a run opens.
///
/// The two are told apart because they are run differently: a command line is split into words, so
/// the program it names is its first and the rest are its arguments, while the editor this project
/// ships is one path and takes the file alone.
#[derive(Debug, Clone)]
pub enum Editor {
    /// A command line — a program and any arguments of its own — as a run or its environment wrote
    /// one, e.g. `code --wait`.
    Named(String),
    /// The `rola-edit` this project ships, sitting beside this program.
    BuiltIn(PathBuf),
}

/// The editor a run opens, as the run resolved one.
///
/// An editor named as an empty string is a run saying it has none: the distinction matters because
/// a run that named none at all is resolved by the environment and then the built-in one, while one
/// that named emptiness is asking to be refused when a command has to open something.
#[derive(Debug, Default, Clone)]
pub struct ResEditor {
    /// The editor to open, or nothing when the run named none it could resolve, or named emptiness.
    editor: Option<Editor>,
    /// Whether `--editor` was given at all, which is what tells naming none from naming emptiness.
    named: bool,
}

impl ResEditor {
    /// The editor to open, or why there is none to open.
    ///
    /// # Errors
    ///
    /// Returns [`ErrorNoEditor`] when neither the run, the environment, nor the built-in editor
    /// named one.
    pub fn must_bind(&self) -> Result<&Editor, ErrorNoEditor> {
        self.editor
            .as_ref()
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

/// A [`ProgramSetup`] that turns `--editor`, the environment and the built-in editor into
/// [`ResEditor`].
///
/// `--editor` comes first — including when it names emptiness, which is a run saying it has none —
/// and the environment is consulted only when the argument is absent, so a run that names an editor
/// is never overridden by one the shell happens to carry. When neither names one, the `rola-edit`
/// this project ships beside this program is used, so a machine with nothing else to edit with still
/// has one.
pub struct EditorSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for EditorSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        let asked = program.pick_argument(&GLOBAL_ARG_EDITOR);
        let named = asked.is_some();

        let editor = match asked {
            Some(written) if !written.is_empty() => Some(Editor::Named(written)),
            // Emptiness is a run saying it has no editor, and nothing else is reached for it.
            Some(_) => None,
            None => named_by_environment().or_else(built_in),
        };

        program.with_resource(ResEditor { editor, named });
    }
}

/// The editor the environment names, or nothing when it names none.
fn named_by_environment() -> Option<Editor> {
    EDITOR_ENV_VARS
        .iter()
        .find_map(|name| env::var(name).ok())
        .filter(|editor| !editor.is_empty())
        .map(Editor::Named)
}

/// The editor this project ships, beside this program, when it is laid down there.
///
/// `rola-edit` is exported beside `rola` (see `./run.sh export`), which is the place a word of this
/// program's reaches a program next to it — `rola edit` runs it the same way. A run that named no
/// editor anywhere falls back here rather than refusing, so a machine with nothing else to edit with
/// still has this.
fn built_in() -> Option<Editor> {
    let here = env::current_exe().ok()?;
    let program = here
        .parent()?
        .join(format!("rola-edit{}", env::consts::EXE_SUFFIX));

    program.is_file().then_some(Editor::BuiltIn(program))
}

/// Opens `editor` on `path`, waiting for it to end.
///
/// A command line is named the way a shell would write one — a program and any arguments of its own
/// — so what is split is words, and `path` is handed on after them. The built-in editor is one path
/// and is not split, because a directory it sits in may have a space in its name. An editor that
/// ends having failed, or that cannot be started at all, is a run that did not get what it asked
/// for.
///
/// # Errors
///
/// Returns [`ErrorEditorFailed`] when the editor names no program, cannot be started, or ends
/// without success.
pub fn open(editor: &Editor, path: &Path) -> Result<(), ErrorEditorFailed> {
    let (program, words, shown) = match editor {
        Editor::Named(command) => {
            let mut words = command.split_whitespace();

            let Some(program) = words.next() else {
                return Err(ErrorEditorFailed {
                    editor: command.clone(),
                    cause: t!("editor.err_empty_program").trim().to_string(),
                });
            };

            (
                program.to_owned(),
                words.map(str::to_owned).collect::<Vec<_>>(),
                command.clone(),
            )
        }
        Editor::BuiltIn(program) => {
            let shown = program.display().to_string();

            (shown.clone(), Vec::new(), shown)
        }
    };

    let status = Command::new(&program).args(&words).arg(path).status();

    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(ErrorEditorFailed {
            editor: shown,
            cause: t!("editor.err_editor_status", status = status.to_string())
                .trim()
                .to_string(),
        }),
        Err(error) => Err(ErrorEditorFailed {
            editor: shown,
            cause: error.to_string(),
        }),
    }
}
