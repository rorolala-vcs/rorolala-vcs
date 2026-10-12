#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
#![allow(clippy::missing_const_for_fn)]

//! The command line is one file to open and nothing else: what a run was going to say is in the
//! file, and everything after opening it is the caller's. The file comes first in the argument
//! list because it is the only thing a run is really about, and the language the run speaks is
//! read the way `rola` reads one — the argument, then the environment, then English.

rust_i18n::i18n!("i18n", fallback = "en");

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use arg_picker::{Picker, PickerArg};
use rust_i18n::{set_locale, t};

mod editor;

use crate::editor::Editor;

/// The file to edit, in the only position it is written at.
///
/// Positional and optional: a run that names no file has asked for nothing, and is told so rather
/// than opened onto an empty one.
///
/// Built through the constructor rather than with `arg!`, unlike the rest of this workspace: the
/// macro resolves its paths through whichever crate `arg-picker` was compiled for, and in a
/// whole-workspace build that is mingling, which this program does not depend on. The constructor
/// is what the macro writes out, and reaching for it keeps this program off mingling.
const FILE_ARG: PickerArg<'static, Option<PathBuf>> = PickerArg::new(&[], None, true);

/// The language this run speaks, in the spelling `rola` names one with.
///
/// Usage: `--lang zh-CN` or `-l zh-CN`.
const LANG_ARG: PickerArg<'static, Option<String>> = PickerArg::new(&["lang"], Some('l'), false);

/// Environment variables consulted, in order, when `--lang` is absent.
///
/// The first one set wins. `ROLA_LANG` is the project's own; `APP_LANG` and `LANG` are the
/// conventions a container and a shell already carry.
const LANGUAGE_ENV_VARS: &[&str] = &["ROLA_LANG", "APP_LANG", "LANG"];

/// Language used when neither `--lang` nor the environment names one.
const FALLBACK_LANGUAGE: &str = "en";

/// What a run ends with when it could not do what it was asked.
const FAILURE: u8 = 1;

/// Why a run did not get what it asked for.
///
/// The two are told apart because a caller reads them differently: a file that will not open is a
/// path to correct, while an editor that will not run is a terminal to look at.
enum Failure {
    /// The file named could not be read.
    File(std::io::Error),
    /// The editor could not be opened, or ended without the file being kept.
    Editor(std::io::Error),
}

fn main() -> ExitCode {
    // Positional first and the flag after it, so the file is claimed by the one position there is
    // before any word is read as the language.
    let (file, language) = Picker::from_args()
        .pick_or_default(&FILE_ARG)
        .pick_or_default(&LANG_ARG)
        .unwrap();

    set_locale(&language.map_or_else(default_language, |value| locale_of(&value)));

    let Some(file) = file else {
        eprintln!("{}", t!("edit.errors.no_file"));
        return ExitCode::from(FAILURE);
    };

    match edit(file) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::File(error)) => {
            eprintln!("{}", t!("edit.errors.file", error = error));
            ExitCode::from(FAILURE)
        }
        Err(Failure::Editor(error)) => {
            eprintln!("{}", t!("edit.errors.editor", error = error));
            ExitCode::from(FAILURE)
        }
    }
}

/// Opens `path` in the editor and lets it run to its end.
///
/// The path is settled before the terminal is touched, so a file that cannot be opened is refused
/// with the shell still in the shape the caller left it in.
///
/// # Errors
///
/// Returns [`Failure::File`] when the path names nothing readable, and [`Failure::Editor`] when
/// the terminal could not be put into the state the editor draws in, or the run ended without the
/// file being written.
fn edit(path: PathBuf) -> Result<(), Failure> {
    let path = precheck(path).map_err(Failure::File)?;
    let mut editor = Editor::open(path).map_err(Failure::File)?;

    editor.run().map_err(Failure::Editor)
}

/// The absolute path `path` names, once it is known to be a file that can be opened.
///
/// A relative path is read against the directory the run was made in, since that is what a caller
/// typing a name means by it. A file that is not there is not created: a run that will not say
/// what it is editing has asked for nothing.
///
/// # Errors
///
/// Returns the reason the path names nothing this can open.
fn precheck(path: PathBuf) -> std::io::Result<PathBuf> {
    let full = if path.is_absolute() {
        path
    } else {
        env::current_dir()?.join(path)
    };

    if !full.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{}", full.display()),
        ));
    }

    if !full.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{}", full.display()),
        ));
    }

    Ok(full)
}

/// The language to speak when `--lang` named none: the environment, or the fallback.
fn default_language() -> String {
    LANGUAGE_ENV_VARS
        .iter()
        .find_map(|name| env::var(name).ok())
        .filter(|language| !language.is_empty())
        .map_or_else(
            || FALLBACK_LANGUAGE.to_string(),
            |language| locale_of(&language),
        )
}

/// An environment value as a locale rust-i18n knows.
///
/// `LANG` and friends carry an encoding suffix and spell the territory with an underscore —
/// `zh_CN.UTF-8` — and neither belongs in a locale name, so they become `zh-CN`. A value already
/// in that shape passes through unchanged.
fn locale_of(value: &str) -> String {
    value.split('.').next().unwrap_or(value).replace('_', "-")
}
