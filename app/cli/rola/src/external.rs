//! Running a program beside this one for a word that names no command.
//!
//! `rola daemon` is `rola-daemon`: what a run writes after this program's name may be the rest of
//! another program's, and the two are exported together — see `./run.sh export`. The word is used
//! as it was written, so a program whose own name has hyphens in it is reached the same way `rola
//! aa-bb-cc` is `rola-aa-bb-cc`. It is one word and not several: `rola aa bb` looks for `rola-aa`
//! and hands it `bb`.
//!
//! Nothing is looked up. The program is looked for where this program was run from, which is where
//! an export lays the programs down and where `rola desktop` reaches for the Desktop program. A
//! `PATH` is the machine's, and a run that picked a program up from one would be running somebody
//! else's program for a word of this one's.
//!
//! What is handed over is the arguments written after that word, as they were written. This
//! program's own flags are not among them: they were taken out of the run before its words were
//! matched, and a flag of this program's is not known to be a flag of the program beside it. A run
//! that asked for help is the one case with something to say about that, and it is answered where
//! help is answered — see [`tell_help`].
//!
//! The program's own output is the terminal's — nothing here reads it, and nothing of this
//! program's is drawn beside it — and this run ends with the code that program ended with, which is
//! what naming a word of this program's means too.

use std::path::{Component, Path, PathBuf};
use std::process::Command;

use mingling::{
    consts::HELP_FLAG,
    hook::{ProgramControls, ProgramHook},
    res::ResExitCode,
};
use rorolala_utils_cli_theme::{err_line, help_line};
use rust_i18n::t;

use crate::ThisProgram;
use crate::exit_codes::EC_ERR_EXTERNAL_FAILED;

/// What a program beside this one is named with: this program's own name and a hyphen.
const PREFIX: &str = "rola-";

/// The hook that hands a word naming no command to a program beside this one.
///
/// It is asked before the words are matched, which is the only place the answer still changes
/// anything: once they are matched, a word that names no command is a mistake being reported, and
/// a run whose output was asked for as a structure never reaches that report at all.
///
/// A run that asked for help is left alone: what it asked for is what the program can do, and that
/// is answered where help is answered, by asking the program itself ([`tell_help`]).
#[must_use]
pub fn hook() -> ProgramHook<ThisProgram> {
    ProgramHook::empty().on_pre_dispatch(|info| {
        if ThisProgram::this().user_context.help {
            return ProgramControls::Empty;
        }

        let Some(program) = named(info.arguments) else {
            return ProgramControls::Empty;
        };

        // The word named the program, so it is not one of the arguments the program is told: what
        // is handed over is everything the run wrote after it.
        let code = hand_over(&program, &info.arguments[1..]);

        // Ended where it stands rather than through a control, because the framework has no control
        // that ends a run: every way out of a hook renders something, and a run that has given its
        // whole output to another program has nothing of its own left to render. What a caller
        // reads is that program's own code, which is what waiting for the run is waiting for.
        std::process::exit(code);
    })
}

/// Answers a run that asked for help by asking the program it named, when it named one.
///
/// A program's help is the program's own to give: this program can say nothing about what another
/// one does, so a run naming one and asking for help is handed over with the question, and the
/// program answers in its own words. What was written after the word is handed over with it, so
/// `rola daemon listen --help` asks the daemon about `listen`.
///
/// Returns whether the run was answered this way, which is what says this program has nothing of
/// its own to add.
///
/// # Panics
///
/// Panics if the framework's own help flag names no long spelling, which is a flag this program
/// could not have been asked for help with either.
#[must_use]
pub fn tell_help(words: &[String], ec: &mut ResExitCode) -> bool {
    let Some(program) = named(words) else {
        return false;
    };

    let mut arguments: Vec<String> = words.iter().skip(1).cloned().collect();
    arguments.push(help_flag());

    ec.exit_code = hand_over(&program, &arguments);

    true
}

/// Runs `program` with `arguments`, and answers what it ended with.
///
/// The program's own streams are the terminal's: nothing here reads them, and nothing of this
/// program's is written beside them. What is written here is the one failure that never reaches the
/// program — one that will not start — and it is written to the error stream rather than into a
/// render buffer, because the run that meets it may have no buffer at all: a word naming a program
/// is answered before anything is rendered.
///
/// A program a signal ended has no code of its own, and is one that did not end well, which is what
/// this run says in its place — the judgement `rola desktop` makes about the window it waits for.
fn hand_over(program: &Path, arguments: &[String]) -> i32 {
    match Command::new(program).args(arguments).status() {
        Ok(status) => status.code().unwrap_or(EC_ERR_EXTERNAL_FAILED),
        Err(error) => {
            eprintln!(
                "{}",
                err_line!(
                    t!(
                        "external.err_launch_failed",
                        program = program.display().to_string(),
                        reason = error.to_string()
                    )
                    .trim()
                )
            );
            eprintln!(
                "{}",
                help_line!(t!("external.err_launch_failed_help").trim())
            );

            EC_ERR_EXTERNAL_FAILED
        }
    }
}

/// The program beside this one that the words name, or nothing when they name none.
///
/// What is read is the first word: it is the one the program is named by, and the ones after it are
/// what it is told.
fn named(words: &[String]) -> Option<PathBuf> {
    let word = words.first()?;

    if !names_a_program(word) || crate::commands().iter().any(|command| command == word) {
        return None;
    }

    let here = std::env::current_exe().ok()?;
    let program = here.parent()?.join(program_name(word));

    program.is_file().then_some(program)
}

/// What a word is looked for beside this program under.
///
/// The word is written after this program's name as it was typed, and the suffix this platform
/// gives a program is added to whole it: `daemon` is `rola-daemon`, and `rola-daemon.exe` where a
/// program is named with one. The name is put together rather than written out once per platform,
/// as the Desktop program's is.
fn program_name(word: &str) -> String {
    format!("{PREFIX}{word}{}", std::env::consts::EXE_SUFFIX)
}

/// How the program beside this one is asked for its help.
///
/// The framework's own flag and not a spelling written here: what is asked of the program is what
/// would have been asked of this one, and a spelling that drifted would be a question the program
/// does not hear.
fn help_flag() -> String {
    // UNWRAP: the flag the framework reads a run's help request out of, which a program that can be
    // asked for help has at least one spelling of.
    let name = HELP_FLAG
        .full
        .first()
        .expect("the help flag is spelled at least once");

    format!("--{name}")
}

/// Whether a word could be the part of another program's name.
///
/// A word beginning with a hyphen is a flag, and one beginning with an underscore is this
/// program's own — the completion the framework reaches through one begins with two, and a version
/// request is rewritten to one before the words are matched — so neither names a program beside
/// this one.
///
/// What is left has to be one name: a word carrying a step (`..`) or a separator is a path rather
/// than a name, and putting one after `rola-` would look outside the directory this program sits
/// in, which is not where a word of this program's reaches.
fn names_a_program(word: &str) -> bool {
    let mut parts = Path::new(word).components();

    !word.starts_with('-')
        && !word.starts_with('_')
        && matches!(parts.next(), Some(Component::Normal(_)))
        && parts.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::{help_flag, names_a_program, program_name};

    /// A word is a name while it is one name, and not a flag or this program's own argument.
    #[test]
    fn a_word_names_a_program_unless_it_is_a_flag_or_a_path() {
        assert!(names_a_program("daemon"));
        assert!(names_a_program("aa-bb-cc"));

        // A flag is not a name, and neither is an argument the framework keeps for itself.
        assert!(!names_a_program("-V"));
        assert!(!names_a_program("--json"));
        assert!(!names_a_program("_version"));
        assert!(!names_a_program("__comp"));
        assert!(!names_a_program(""));

        // A path is not a name: what follows the prefix has to be one part of one.
        assert!(!names_a_program(".."));
        assert!(!names_a_program("."));
        assert!(!names_a_program("../daemon"));
        assert!(!names_a_program("daemon/../x"));
        assert!(!names_a_program(r"..\daemon"));
    }

    /// The word is written after this program's name whole, hyphens and all.
    #[test]
    fn a_word_is_written_after_this_programs_name() {
        let suffix = std::env::consts::EXE_SUFFIX;

        assert_eq!(program_name("daemon"), format!("rola-daemon{suffix}"));
        assert_eq!(program_name("aa-bb-cc"), format!("rola-aa-bb-cc{suffix}"));
    }

    /// Help is asked for with the flag the framework reads a help request out of.
    #[test]
    fn help_is_asked_for_with_the_frameworks_own_flag() {
        assert_eq!(help_flag(), "--help");
    }
}
