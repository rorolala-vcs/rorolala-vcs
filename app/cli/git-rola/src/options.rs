//! What a command line asks for before any command runs.
//!
//! git reads its command line in two steps: the first word names a command, and the words after it
//! belong to that command. The framework's global flags cannot express that, because they are
//! picked out of the whole argument list and stop at no command boundary, so the judgement is made
//! here, before the dispatch tree sees the words: `git rola check -h` is check's usage, `git rola
//! check -v` is a flag check does not have, and `git rola restore -- -h` hands `-h` to restore as a
//! name.
//!
//! The rules are git's: `-h` before `--` is the command's usage, written on stdout with status 129;
//! `--help` is that usage with status 0 when it is the command's first word and with 129 anywhere
//! else; a flag the command does not have is `error:` on stderr with 129; `--` ends the flags. At
//! the root, a word no command answers to is a command line git would have refused, with status 1,
//! and a flag no command has is `unknown option:` with 129.
//!
//! Writing the answer is [`draw`]'s: these are the answers that no command gives, so they are
//! written here and end the run, the way git's own `usage()` and `help` paths do.

use std::io::Write as _;

use rust_i18n::t;

use crate::exit_codes::{EC_FAILED, EC_OK, EC_USAGE};
use crate::usage;
use crate::wrap;

/// How far a mistyped word may be from a command and still be offered as one it may have meant.
const MAX_EDITS: usize = 2;

/// How many commands a mistyped word is offered at most.
const MAX_GUESSES: usize = 3;

/// What the words of a run come to.
pub enum Decision {
    /// The run names a command this program has, and its entry answers it.
    Dispatch,
    /// The run names a command git has, and git answers it.
    Wrap {
        /// The command git is run with.
        verb: String,
        /// The words after it, handed to git unchanged.
        rest: Vec<String>,
    },
    /// The run asks something no command can answer.
    Draw(Answer),
}

/// One answer written before any command runs.
pub enum Answer {
    /// The program's usage, with the status git ends such a run with.
    ProgramUsage {
        /// The status the run ends with.
        code: i32,
    },
    /// A command's usage, with the status git ends such a run with.
    CommandUsage {
        /// The command whose usage is written.
        command: &'static str,
        /// The status the run ends with.
        code: i32,
    },
    /// The version line.
    Version,
    /// A word that names no command, with the commands it may have meant.
    UnknownCommand {
        /// The word as it was typed.
        typed: String,
    },
    /// A flag at the program's root that no command has.
    UnknownOption {
        /// The flag as it was typed.
        word: String,
    },
    /// A flag a command does not have, answered with that command's usage.
    CommandRefusal {
        /// The command the flag was written for.
        command: &'static str,
        /// What was wrong with it.
        reason: Refusal,
    },
}

/// Why a command's line is refused.
pub enum Refusal {
    /// A long flag the command does not have.
    UnknownOption {
        /// The name without its dashes, as git reports it.
        word: String,
    },
    /// A short flag the command does not have.
    UnknownSwitch {
        /// The letter, as git reports it.
        word: String,
    },
}

/// What `words` ask for.
pub fn decide(words: &[String]) -> Decision {
    let Some(first) = words.first().map(String::as_str) else {
        // A bare `git rola` is git's bare `git`: the usage on stdout, status 1. It is not a
        // command line that could not be read, so it is not 129.
        return Decision::Draw(Answer::ProgramUsage { code: EC_FAILED });
    };

    if matches!(first, "-h" | "--help") {
        return Decision::Draw(at_the_root(&words[1..]));
    }

    // git answers the version whatever follows it, so the words after it are not read at all.
    if matches!(first, "-v" | "--version") {
        return Decision::Draw(Answer::Version);
    }

    // git has no other global flag at this level: anything else with a dash is a flag no command
    // has, and git says so without an `error:` prefix.
    if first.starts_with('-') {
        return Decision::Draw(Answer::UnknownOption {
            word: first.to_owned(),
        });
    }

    if wrap::WRAPPED.contains(&first) {
        return Decision::Wrap {
            verb: first.to_owned(),
            rest: words[1..].to_vec(),
        };
    }

    if first == "help" {
        return decide_help(&words[1..]);
    }

    let Some(command) = usage::named(first) else {
        return Decision::Draw(Answer::UnknownCommand {
            typed: first.to_owned(),
        });
    };

    decide_command(command, &words[1..])
}

/// The operands of a command's words: everything but the end-of-options marker.
///
/// The marker itself is dropped and everything after it is kept, including words that look like
/// flags, which is what `--` means. The framework's picker stops at the marker instead, which is
/// right for a command's own flags but leaves a git-style path after `--` unread, so a command
/// takes its operands from here rather than through a pick.
pub fn operands(words: &[String]) -> Vec<String> {
    let mut operands = Vec::with_capacity(words.len());
    let mut after_marker = false;

    for word in words {
        if !after_marker && word == "--" {
            after_marker = true;

            continue;
        }

        operands.push(word.clone());
    }

    operands
}

/// The answer a root `-h` or `--help` asks for.
///
/// `git rola -h check` is git's `git -h check`, which is read as `git help check`: the word after
/// the flag names the command whose usage is wanted. A word that names no command leaves the
/// program's own usage, since there is no command to write down.
fn at_the_root(rest: &[String]) -> Answer {
    rest.first().and_then(|word| usage::named(word)).map_or(
        Answer::ProgramUsage { code: EC_OK },
        |command| Answer::CommandUsage {
            command,
            code: EC_OK,
        },
    )
}

/// The answer `help` asks for, from the words after it.
///
/// `help` is a command of its own, so its flags are read the way a command's are, and its first
/// operand names the command to write down. A later operand is ignored, the way `git help status
/// extra` ignores it. A word that names no command is refused here rather than dispatched, since
/// the node only ever sees a name it has.
fn decide_help(rest: &[String]) -> Decision {
    let mut spoken: Vec<&String> = Vec::new();

    for (index, word) in rest.iter().enumerate() {
        if word == "--" {
            break;
        }

        if let Some(answer) = asked(word, index == 0, "help") {
            return Decision::Draw(answer);
        }

        spoken.push(word);
    }

    match spoken.first() {
        None => Decision::Dispatch,
        Some(word) if usage::named(word).is_some() => Decision::Dispatch,
        Some(word) => Decision::Draw(Answer::UnknownCommand {
            typed: (*word).clone(),
        }),
    }
}

/// The answer a command's words ask for before the command runs, if they ask one.
fn decide_command(command: &'static str, rest: &[String]) -> Decision {
    for (index, word) in rest.iter().enumerate() {
        // Everything after the marker is an operand, `-h` included: that is what the marker is for.
        if word == "--" {
            break;
        }

        if let Some(answer) = asked(word, index == 0, command) {
            return Decision::Draw(answer);
        }
    }

    Decision::Dispatch
}

/// The answer `word` asks for when a command reads it, if it asks one.
///
/// `first` is whether the word is the command's own first word, which is the only place git reads
/// `--help` as a request for the documentation rather than as a mistake.
fn asked(word: &str, first: bool, command: &'static str) -> Option<Answer> {
    match word {
        "-h" => Some(Answer::CommandUsage {
            command,
            code: EC_USAGE,
        }),
        "--help" => Some(Answer::CommandUsage {
            command,
            code: if first { EC_OK } else { EC_USAGE },
        }),
        _ => named_flag(word).map(|reason| Answer::CommandRefusal { command, reason }),
    }
}

/// Why `word` is a flag no command has, when it is flag-shaped at all.
///
/// git names a long flag without its dashes and up to its `=`, and a short one by its first letter,
/// since a cluster of them is read one letter at a time. A lone `-` is a path, not a flag.
fn named_flag(word: &str) -> Option<Refusal> {
    if let Some(long) = word.strip_prefix("--") {
        return Some(Refusal::UnknownOption {
            word: long.split('=').next().unwrap_or(long).to_owned(),
        });
    }

    if word.len() > 1 && word.starts_with('-') {
        return Some(Refusal::UnknownSwitch {
            word: word.chars().nth(1).map_or_else(String::new, String::from),
        });
    }

    None
}

/// Writes the answer and ends the run with the status it carries.
pub fn draw(answer: Answer) -> ! {
    let code = match answer {
        Answer::ProgramUsage { code } => {
            say_stdout(&format!("{}\n", usage::program()));

            code
        }
        Answer::CommandUsage { command, code } => {
            say_stdout(&format!("{}\n", usage::command(command)));

            code
        }
        Answer::Version => {
            let version = env!("CARGO_PKG_VERSION");

            say_stdout(&format!("{}\n", t!("git_rola.version", version = version)));

            EC_OK
        }
        Answer::UnknownCommand { typed } => {
            say_stderr(&format!(
                "{}\n",
                t!("git_rola.err_unknown_command", typed = typed.as_str())
            ));
            draw_guesses(&typed);

            EC_FAILED
        }
        Answer::UnknownOption { word } => {
            say_stderr(&format!(
                "{}\n",
                t!("git_rola.err_unknown_option", word = word.as_str())
            ));
            say_stderr(&format!("{}\n", usage::program()));

            EC_USAGE
        }
        Answer::CommandRefusal { command, reason } => {
            let said = match reason {
                Refusal::UnknownOption { word } => {
                    t!("git_rola.err_no_such_option", word = word.as_str())
                }
                Refusal::UnknownSwitch { word } => {
                    t!("git_rola.err_no_such_switch", word = word.as_str())
                }
            };

            say_stderr(&format!("{}{said}\n", t!("message.error")));
            say_stderr(&format!("{}\n", usage::command(command)));

            EC_USAGE
        }
    };

    std::process::exit(code);
}

/// Says what a mistyped word may have meant, in git's shape.
fn draw_guesses(typed: &str) {
    let guessed = similar(typed);

    if guessed.is_empty() {
        return;
    }

    // git writes a blank line, the sentence, then one tab-indented name per line.
    let heading = if guessed.len() == 1 {
        t!("git_rola.similar_command")
    } else {
        t!("git_rola.similar_commands")
    };

    say_stderr(&format!("\n{heading}\n"));

    for name in &guessed {
        say_stderr(&format!("\t{name}\n"));
    }
}

/// Writes `text` to standard output, leaving a failure to the reader.
///
/// This is the answer a run gives before any command, which git also writes without checking: the
/// result contract belongs to a command, and there is none here.
fn say_stdout(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(text.as_bytes());
    let _ = out.flush();
}

/// Writes `text` to standard error, leaving a failure to the reader, see [`say_stdout`].
fn say_stderr(text: &str) {
    let mut err = std::io::stderr().lock();
    let _ = err.write_all(text.as_bytes());
    let _ = err.flush();
}

/// The commands a mistyped word may have meant, nearest first.
///
/// The scoring is `rola`'s, copied from `app/cli/rola/src/main.rs` where a mistyped rola command is
/// guessed at: the two programs answer the same way for the same word, so the same typo is offered
/// the same command in both.
fn similar(typed: &str) -> Vec<String> {
    let mut close: Vec<(usize, String)> = usage::COMMANDS
        .iter()
        .filter_map(|command| closeness(typed, command).map(|edits| (edits, (*command).to_owned())))
        .collect();

    // Nearest first, and in name order between the equally near, which is what a reader comparing
    // two guesses wants.
    close.sort();
    close.truncate(MAX_GUESSES);

    close.into_iter().map(|(_, command)| command).collect()
}

/// How many edits separate `typed` from naming `command`, when it is close enough to be worth
/// offering.
fn closeness(typed: &str, command: &str) -> Option<usize> {
    if typed.is_empty() {
        return None;
    }

    if command.starts_with(typed) || typed.starts_with(command) {
        return Some(0);
    }

    let edits = edit_distance(typed, command);

    (edits <= MAX_EDITS).then_some(edits)
}

/// How many single-character edits turn `left` into `right`.
fn edit_distance(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();

    for (row, left_char) in left.chars().enumerate() {
        let mut current = Vec::with_capacity(right.len() + 1);
        current.push(row + 1);

        for (column, right_char) in right.iter().enumerate() {
            let changed = previous[column] + usize::from(left_char != *right_char);
            let added = current[column] + 1;
            let dropped = previous[column + 1] + 1;

            current.push(changed.min(added).min(dropped));
        }

        previous = current;
    }

    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::{Answer, Decision, Refusal, decide, edit_distance, operands, similar};

    /// What `words` come to, said as one line.
    fn answer(words: &[&str]) -> String {
        let words: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();

        match decide(&words) {
            Decision::Dispatch => "dispatch".to_owned(),
            Decision::Wrap { verb, .. } => format!("wrap {verb}"),
            Decision::Draw(Answer::ProgramUsage { code }) => format!("program usage {code}"),
            Decision::Draw(Answer::CommandUsage { command, code }) => {
                format!("{command} usage {code}")
            }
            Decision::Draw(Answer::Version) => "version".to_owned(),
            Decision::Draw(Answer::UnknownCommand { typed }) => format!("unknown command {typed}"),
            Decision::Draw(Answer::UnknownOption { word }) => format!("unknown option {word}"),
            Decision::Draw(Answer::CommandRefusal { command, reason }) => {
                let reason = match reason {
                    Refusal::UnknownOption { word } => format!("option {word}"),
                    Refusal::UnknownSwitch { word } => format!("switch {word}"),
                };

                format!("refused {command} {reason}")
            }
        }
    }

    #[test]
    fn a_run_that_names_nothing_asks_for_the_usage() {
        assert_eq!(answer(&[]), "program usage 1");
    }

    #[test]
    fn help_at_the_root_is_a_question_about_the_program() {
        assert_eq!(answer(&["-h"]), "program usage 0");
        assert_eq!(answer(&["--help"]), "program usage 0");
        assert_eq!(answer(&["-h", "check"]), "check usage 0");
        assert_eq!(
            answer(&["-h", "nosuch"]),
            "program usage 0",
            "a word that names no command leaves the program's own usage"
        );
    }

    #[test]
    fn the_version_ignores_what_follows_it() {
        assert_eq!(answer(&["-v"]), "version");
        assert_eq!(answer(&["--version"]), "version");
        assert_eq!(answer(&["--version", "status"]), "version");
    }

    #[test]
    fn a_flag_no_command_has_is_a_flag_no_command_has() {
        assert_eq!(answer(&["--bogus"]), "unknown option --bogus");
        assert_eq!(answer(&["--"]), "unknown option --");
        assert_eq!(answer(&["--bogus", "check"]), "unknown option --bogus");
    }

    #[test]
    fn a_word_that_names_no_command_is_refused() {
        assert_eq!(answer(&["nosuch"]), "unknown command nosuch");
    }

    #[test]
    fn a_command_alone_is_dispatched() {
        for command in ["init", "hook", "check", "restore", "update", "version"] {
            assert_eq!(answer(&[command]), "dispatch", "`{command}` alone");
        }
    }

    #[test]
    fn a_command_reads_its_own_flags() {
        assert_eq!(answer(&["check", "-h"]), "check usage 129");
        assert_eq!(answer(&["check", "--help"]), "check usage 0");
        assert_eq!(answer(&["check", "x", "--help"]), "check usage 129");
        assert_eq!(answer(&["restore", "-h"]), "restore usage 129");
        assert_eq!(answer(&["restore", "--help"]), "restore usage 0");
        assert_eq!(answer(&["restore", "x", "--help"]), "restore usage 129");
    }

    #[test]
    fn an_unknown_flag_ends_a_command_before_a_later_help_does() {
        assert_eq!(answer(&["check", "--bogus"]), "refused check option bogus");
        assert_eq!(
            answer(&["check", "--bogus", "-h"]),
            "refused check option bogus",
            "what comes first is what is answered"
        );
        assert_eq!(answer(&["check", "-h", "--bogus"]), "check usage 129");
    }

    #[test]
    fn a_short_flag_is_named_by_its_letter_and_a_long_one_by_its_name() {
        for command in [
            "check", "restore", "hook", "init", "update", "version", "help",
        ] {
            assert_eq!(
                answer(&[command, "-C"]),
                format!("refused {command} switch C"),
                "`{command} -C`"
            );
            assert_eq!(
                answer(&[command, "--no-such=1"]),
                format!("refused {command} option no-such"),
                "`{command} --no-such=1`"
            );
        }
    }

    #[test]
    fn everything_after_the_marker_is_an_operand() {
        assert_eq!(answer(&["restore", "--", "-h"]), "dispatch");
        assert_eq!(answer(&["restore", "--", "--help"]), "dispatch");
        assert_eq!(answer(&["check", "--", "-h"]), "dispatch");
    }

    #[test]
    fn a_lone_dash_is_a_path() {
        assert_eq!(answer(&["restore", "-"]), "dispatch");
    }

    #[test]
    fn a_wrapped_command_is_handed_over_unread() {
        assert_eq!(answer(&["switch", "-C", "x"]), "wrap switch");
        assert_eq!(answer(&["switch", "--help"]), "wrap switch");
        assert_eq!(answer(&["checkout", "--", "-h"]), "wrap checkout");
        assert_eq!(answer(&["merge"]), "wrap merge");
        assert_eq!(answer(&["pull"]), "wrap pull");
    }

    #[test]
    fn help_is_a_command_of_its_own() {
        assert_eq!(answer(&["help"]), "dispatch");
        assert_eq!(answer(&["help", "check"]), "dispatch");
        assert_eq!(answer(&["help", "check", "extra"]), "dispatch");
        assert_eq!(answer(&["help", "-h"]), "help usage 129");
        assert_eq!(answer(&["help", "--help"]), "help usage 0");
        assert_eq!(answer(&["help", "--bogus"]), "refused help option bogus");
    }

    #[test]
    fn a_help_for_a_word_that_names_no_command_is_refused() {
        assert_eq!(answer(&["help", "nosuch"]), "unknown command nosuch");
    }

    #[test]
    fn the_version_command_ignores_its_operands() {
        assert_eq!(answer(&["version", "extra"]), "dispatch");
        assert_eq!(answer(&["version", "-h"]), "version usage 129");
        assert_eq!(
            answer(&["version", "--bogus"]),
            "refused version option bogus"
        );
    }

    #[test]
    fn the_marker_is_dropped_and_what_follows_it_is_kept() {
        let words = |words: &[&str]| -> Vec<String> {
            words.iter().map(|word| (*word).to_owned()).collect()
        };

        assert_eq!(operands(&words(&[])), Vec::<String>::new());
        assert_eq!(operands(&words(&["a", "b"])), words(&["a", "b"]));
        assert_eq!(operands(&words(&["--", "-h"])), words(&["-h"]));
        assert_eq!(operands(&words(&["a", "--", "-h"])), words(&["a", "-h"]));
        assert_eq!(
            operands(&words(&["--", "a", "--"])),
            words(&["a", "--"]),
            "only the first marker is syntax"
        );
    }

    #[test]
    fn an_edit_is_a_character_added_dropped_or_changed() {
        assert_eq!(edit_distance("check", "check"), 0);
        assert_eq!(edit_distance("chek", "check"), 1);
        assert_eq!(edit_distance("checkk", "check"), 1);
        assert_eq!(edit_distance("", "check"), 5);
    }

    #[test]
    fn a_mistyped_word_is_offered_the_command_it_meant() {
        assert_eq!(similar("chek"), ["check"]);
        assert_eq!(similar("restroe"), ["restore"]);
        assert_eq!(similar("nosuch"), Vec::<String>::new());
    }
}
