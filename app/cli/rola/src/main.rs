#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
#![allow(clippy::missing_const_for_fn)]

rust_i18n::i18n!("i18n", fallback = "en");

use std::collections::BTreeSet;

use mingling::{
    macros::{buffer, gen_program, help, r_append, r_eprintln, renderer},
    res::ResExitCode,
    setup::DefaultSetup,
};
use rorolala_cli_setups::RorolalaSetup;
use rorolala_utils_cli_theme::{err_line, help_line, trd};
use rust_i18n::t;

mod account;
mod address;
mod cmd_account;
mod cmd_create;
mod cmd_explain;
mod cmd_init;
mod cmd_pack;
mod cmd_vault;
mod error;
mod exit_codes;
mod keys;
mod lastec;
mod tools;
mod user;

use crate::account::CurrentAccountSetup;
use crate::address::AddressHistorySetup;
use crate::exit_codes::{EC_HELP, EC_UNKNOWN_COMMAND};
use crate::lastec::LastExitCodeRecordSetup;

/// How far a mistyped word may be from a command and still be offered as one it may have
/// meant.
const MAX_EDITS: usize = 2;

/// How many commands a mistyped word is offered at most.
///
/// A guess is only worth making while it is worth reading: past a few, the reader is being
/// shown the program's commands rather than their own mistake.
const MAX_GUESSES: usize = 3;

fn main() {
    let mut program = ThisProgram::new();
    program.with_setup(DefaultSetup);
    program.with_setup(RorolalaSetup);
    program.with_setup(AddressHistorySetup);
    program.with_setup(CurrentAccountSetup);
    program.with_setup(LastExitCodeRecordSetup);
    program.exec_and_exit();
}

/// Prints the help a run falls back to when no command is named.
///
/// A run whose arguments name no command — `rola`, or `rola -h` — reaches no entry of its
/// own, so it lands on the fallback. What it is shown there is this: the whole of what the
/// program can do.
#[help(buffer)]
pub(crate) fn help_rola(_: EntryFallback, ec: &mut ResExitCode) {
    r_eprintln!("{}", trd!(t!("rola.help")).trim());
    ec.exit_code = EC_HELP;
}

/// Reports a run whose arguments named no command the program has.
///
/// A run that named nothing asked for nothing in particular, so it is shown what there is to
/// ask for, the way `rola -h` shows it. One that named something is told there is no such
/// command, and offered the ones it may have meant: a mistyped command costs one word to try
/// again with, so guessing at it is worth more than a list is.
#[renderer(buffer)]
pub(crate) fn render_fallback(args: EntryFallback, ec: &mut ResExitCode) {
    // Naming nothing is not a mistake to report, so it is answered the way asking for help
    // is, and nothing else is said after it.
    if args.is_empty() {
        r_append!(help_rola(args, ec));
    } else {
        // What was typed is reported as it was typed; a command, though, is named by the
        // first word, which is the one that failed to match.
        let typed = args.join(" ");
        let mistyped = args.first().map_or("", String::as_str);

        r_eprintln!(
            "{}",
            err_line!(t!("rola.err_unknown_command", command = typed).trim())
        );

        let guessed = similar_commands(mistyped, &commands());
        if guessed.is_empty() {
            r_eprintln!("{}", help_line!(t!("rola.err_unknown_command_help").trim()));
        } else {
            // The guesses are what could have been typed instead, so they are named the way
            // they would be written: the words themselves are the program's, not English.
            let offered = guessed.join(", ");

            r_eprintln!(
                "{}",
                help_line!(t!("rola.err_unknown_command_similar", commands = offered).trim())
            );
        }

        ec.exit_code = EC_UNKNOWN_COMMAND;
    }
}

/// Each command the program has, by the name it is reached by.
///
/// The names are the program's own, so they are what could have been typed: a node the
/// program keeps to itself is not read at all, and a command with subcommands is reached by
/// the first word of its path, which is the word that has to match.
fn commands() -> Vec<String> {
    ThisProgram::this()
        .get_nodes()
        .into_iter()
        .filter(|(node, _)| !node.is_empty() && !node.starts_with('_'))
        .filter_map(|(node, _)| node.split(' ').next().map(str::to_string))
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect()
}

/// The commands a mistyped word may have meant, nearest first.
///
/// At most [`MAX_GUESSES`] of them, since a longer list is the program's commands rather than
/// a guess at what was meant.
fn similar_commands(mistyped: &str, commands: &[String]) -> Vec<String> {
    let mut close: Vec<(usize, String)> = commands
        .iter()
        .filter_map(|command| closeness(mistyped, command).map(|edits| (edits, command.clone())))
        .collect();

    // Nearest first, and in name order between the equally near, which is what a reader
    // comparing two guesses wants.
    close.sort();
    close.truncate(MAX_GUESSES);

    close.into_iter().map(|(_, command)| command).collect()
}

/// How many edits separate `mistyped` from naming `command`, when it is close enough to be
/// worth offering.
///
/// A word that spells the start of a command is not a mistake but half of one, so a prefix
/// counts however much longer the command is; anything else has to be within [`MAX_EDITS`]
/// edits of it, which is what a typo is.
fn closeness(mistyped: &str, command: &str) -> Option<usize> {
    if mistyped.is_empty() {
        return None;
    }

    if command.starts_with(mistyped) || mistyped.starts_with(command) {
        return Some(0);
    }

    let edits = edit_distance(mistyped, command);
    (edits <= MAX_EDITS).then_some(edits)
}

/// How many single-character edits turn `left` into `right`.
///
/// The number of edits is what a typo costs to fix: a character added, dropped or changed is
/// one each, so a word one edit away is one keystroke from what was meant.
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
    use super::{closeness, edit_distance, similar_commands};

    /// The commands a run can name, as a program would list them.
    fn commands() -> Vec<String> {
        ["account", "create", "init", "key", "vault"]
            .map(str::to_string)
            .to_vec()
    }

    #[test]
    fn an_edit_is_a_character_added_dropped_or_changed() {
        assert_eq!(edit_distance("vault", "vault"), 0);
        assert_eq!(edit_distance("vnult", "vault"), 1);
        assert_eq!(edit_distance("vaultt", "vault"), 1);
        assert_eq!(edit_distance("vault", ""), 5);
    }

    #[test]
    fn a_half_typed_command_is_offered_and_so_is_a_typo() {
        // Half of a word is not a mistake: it is the command, typed so far.
        assert_eq!(closeness("v", "vault"), Some(0));
        assert_eq!(closeness("acc", "account"), Some(0));

        // A typo is measured by what it costs to fix.
        assert_eq!(closeness("vnult", "vault"), Some(1));
        assert_eq!(closeness("it", "init"), Some(2));

        // A word that resembles none of them is not guessed at, and naming nothing is not
        // offered anything.
        assert_eq!(closeness("bogus", "vault"), None);
        assert_eq!(closeness("", "vault"), None);
    }

    #[test]
    fn a_mistyped_command_is_answered_with_the_ones_nearest_it() {
        assert_eq!(similar_commands("vnult", &commands()), ["vault"]);
        assert_eq!(similar_commands("bogus", &commands()), Vec::<String>::new());

        // Half of one command spells no other, so it is the only guess.
        assert_eq!(similar_commands("v", &commands()), ["vault"]);

        // Equally near guesses are given in name order, and there are only ever a few.
        let many: Vec<String> = ["aac", "aad", "aaa", "aab"].map(str::to_string).to_vec();
        assert_eq!(similar_commands("aax", &many), ["aaa", "aab", "aac"]);
    }
}

gen_program!();
