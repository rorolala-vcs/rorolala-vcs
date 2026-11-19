//! What the program says it can do, and how each command is written down.
//!
//! The text is the translation files': the root usage is one block holding the commands and the
//! descriptions they are listed by, and each command's own usage line is a key of its own. The
//! names are git's, so they stay untranslated; only the words around them follow the language.
//!
//! The list is a constant rather than a walk of the dispatch tree, because the order it is printed
//! in is the program's and the tree's order is the registration's. A test compares the two sets, so
//! a command added to one and not the other is reported instead of quietly missing from the list.

use rust_i18n::t;

/// The commands the program has, in the order the root usage lists them.
pub const COMMANDS: [&str; 12] = [
    "init", "hook", "check", "entries", "bring", "upstream", "restore", "refresh", "update",
    "bind", "help", "version",
];

/// The name of a command, when `word` is one.
pub fn named(word: &str) -> Option<&'static str> {
    COMMANDS.iter().copied().find(|command| *command == word)
}

/// The program's usage: what it can do, as `git rola` alone prints it.
///
/// The block ends with the newline the translation states and the listing keeps it, so what is
/// answered is what the file holds; a caller writing it out adds no line of its own.
pub fn program() -> String {
    t!("git_rola.usage", usage = t!("message.usage"))
        .trim_end()
        .to_owned()
}

/// How one command is written down: its usage line under the program's prefix.
///
/// A word that names no command is not a command's usage, so the program's is answered instead;
/// callers name a command they have already read from the argument line.
pub fn command(name: &str) -> String {
    let line = match name {
        "init" => t!("init.usage"),
        "hook" => t!("hook.usage"),
        "check" => t!("check.usage"),
        "entries" => t!("entries.usage"),
        "bring" => t!("bring.usage"),
        "upstream" => t!("upstream.usage"),
        "restore" => t!("restore.usage"),
        "refresh" => t!("refresh.usage"),
        "update" => t!("update.usage"),
        "bind" => t!("bind.usage"),
        "help" => t!("help.usage"),
        "version" => t!("version.usage"),
        _ => return program(),
    };

    format!("{}{}", t!("message.usage"), line)
}

#[cfg(test)]
mod tests {
    use mingling::ProgramCollect as _;

    use super::{COMMANDS, command, named, program};

    /// The commands the dispatch tree answers to.
    fn dispatched() -> Vec<String> {
        let mut nodes: Vec<String> = crate::ThisProgram::get_nodes()
            .into_iter()
            .filter(|(node, _)| !node.is_empty() && !node.starts_with('_'))
            .filter_map(|(node, _)| node.split(' ').next().map(str::to_owned))
            .collect();

        nodes.sort();
        nodes.dedup();
        nodes
    }

    #[test]
    fn the_list_and_the_dispatch_tree_name_the_same_commands() {
        let mut listed: Vec<String> = COMMANDS.iter().map(|name| (*name).to_owned()).collect();

        listed.sort();

        assert_eq!(listed, dispatched());
    }

    #[test]
    fn the_root_usage_lists_every_command() {
        let text = program();

        for name in COMMANDS {
            assert!(text.contains(name), "`{name}` is not listed:\n{text}");
        }

        assert!(text.starts_with(&*rust_i18n::t!("message.usage")));
    }

    #[test]
    fn a_command_usage_is_the_prefix_and_the_line() {
        let text = command("restore");

        assert!(text.starts_with(&*rust_i18n::t!("message.usage")));
        assert!(text.ends_with("git rola restore <path>..."));
    }

    #[test]
    fn the_programs_own_commands_are_written_down_as_git_writes_them() {
        assert_eq!(command("init"), "usage: git rola init");
        assert_eq!(command("check"), "usage: git rola check");
        assert_eq!(command("update"), "usage: git rola update");
        assert_eq!(command("help"), "usage: git rola help [<command>]");
        assert_eq!(command("version"), "usage: git rola version");
    }

    #[test]
    fn a_name_that_is_no_command_has_no_usage_of_its_own() {
        assert_eq!(named("check"), Some("check"));
        assert_eq!(named("nosuch"), None);
        assert_eq!(command("nosuch"), program());
    }
}
