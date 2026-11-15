//! The little of git's configuration this program reads, asked of git itself.
//!
//! A value is not read here from the files: git already knows how its configuration is assembled,
//! including the `GIT_CONFIG_PARAMETERS` a `git -c` leaves behind for the commands it runs, so a
//! question is asked of git and the answer is used as it comes. That keeps `git -c core.quotePath=
//! false rola check` reading the setting the same way `git status` would.

use std::process::{Command, Stdio};

/// The value `key` holds, when it holds one.
///
/// A key with no value answers with success and nothing, which is an empty value rather than an
/// absent one and is reported as such.
pub fn get(key: &str) -> Option<String> {
    let output = ask(&["config", "--get", key])?;

    Some(last_line(&output))
}

/// The value `key` holds read as a boolean, when it holds one.
pub fn boolean(key: &str) -> Option<bool> {
    match last_line(&ask(&["config", "--type=bool", "--get", key])?).as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// What `git var name` answers, when it answers.
///
/// `git var GIT_PAGER` is git's own reading of the pager chain, so asking it is what makes this
/// program page where git would page, `core.pager`, `PAGER` and the built-in default included.
pub fn var(name: &str) -> Option<String> {
    let output = ask(&["var", name])?;
    let value = last_line(&output);

    (!value.is_empty()).then_some(value)
}

/// What git says to `arguments`, when it says something and ends successfully.
fn ask(arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(arguments)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The last line of `text`, without its line ending.
///
/// A configuration value may hold a trailing newline of its own, and git writes one after the value
/// when it answers; what a caller wants is the value, so only the final line ending is dropped.
fn last_line(text: &str) -> String {
    let without_newline = text.strip_suffix('\n').unwrap_or(text);
    let without_carriage_return = without_newline
        .strip_suffix('\r')
        .unwrap_or(without_newline);

    without_carriage_return.to_owned()
}

#[cfg(test)]
mod tests {
    use super::last_line;

    #[test]
    fn the_line_ending_git_writes_is_not_part_of_the_value() {
        assert_eq!(last_line("less\n"), "less");
        assert_eq!(last_line("less\r\n"), "less");
        assert_eq!(last_line(""), "");
    }

    #[test]
    fn a_value_that_ends_in_a_newline_keeps_the_rest() {
        assert_eq!(last_line("a b\n"), "a b");
        assert_eq!(last_line("\n"), "");
    }
}
