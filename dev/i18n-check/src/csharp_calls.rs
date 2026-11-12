//! Finds the keys a C# program's `I18n.Get` calls name.
//!
//! There is no C# parser here, so the calls are found by their spelling: a call to `Get` on a
//! receiver whose name mentions `i18n` (`RolaI18N`, `_i18n`, `i18n`, `_services.I18n`) whose
//! first argument is a string literal. A call made on anything else, or with an argument that is
//! not a literal, is not read.

use std::fs;
use std::path::Path;

use crate::calls::{Call, Calls};
use crate::files;

/// The call a key is named in.
const GET: &str = ".Get(";

/// The namespace a menu path's keys live in, which is shared by every plugin rather than owned by one.
pub const SHARED_MENU: &str = "menu.entry.";

/// Reads every `.cs` file under `dir` and collects the keys its `I18n.Get` calls name.
///
/// A build tree under the sources (`bin`, `obj`) is left alone, since what it holds is a copy of
/// what is already read.
///
/// # Errors
///
/// Returns a message naming a file that could not be read.
pub fn calls(dir: &Path) -> Result<Calls, String> {
    let mut all = Calls::default();

    for file in files::under(dir, &["cs"], &["bin", "obj"])? {
        let text =
            fs::read_to_string(&file).map_err(|error| format!("{}: {error}", file.display()))?;

        scan(&text, &file, &mut all);
    }

    Ok(all)
}

/// Reads every menu path named by a string literal under `dir`.
///
/// A menu path is not a `Get` call: it is a list of keys handed to a menu renderer, which is why the
/// literals are read on their own. Only the shared namespace is read, since that is the one a plugin
/// may use a key from without stating it itself.
///
/// # Errors
///
/// Returns a message naming a file that could not be read.
pub fn menu_keys(dir: &Path) -> Result<Calls, String> {
    let mut all = Calls::default();

    for file in files::under(dir, &["cs"], &["bin", "obj"])? {
        let text =
            fs::read_to_string(&file).map_err(|error| format!("{}: {error}", file.display()))?;

        scan_menu(&text, &file, &mut all);
    }

    Ok(all)
}

/// A menu path begins with a quote and this namespace.
const MENU_QUOTE: &str = "\"menu.entry.";

/// Scans one file's text for `I18n.Get` calls.
fn scan(text: &str, file: &Path, calls: &mut Calls) {
    let mut at = 0;

    while let Some(found) = text[at..].find(GET) {
        let dot = at + found;

        if receiver(text, dot).to_lowercase().contains("i18n") {
            let after = dot + GET.len();
            let rest = text[after..].trim_start();

            match literal(rest) {
                Some(key) => calls.calls.push(Call {
                    file: file.to_path_buf(),
                    line: line(text, dot),
                    key,
                }),
                // A call with no argument at all names nothing; anything else is a key this scan
                // cannot read.
                None if rest.starts_with(')') => {}
                None => calls.unchecked += 1,
            }
        }

        at = dot + GET.len();
    }
}

/// Scans one file's text for string literals that begin a menu path.
///
/// The path a menu entry sits at is written as a list of string literals rather than passed to
/// `Get`, so the literals are read where they are written. An interpolated string is skipped by the
/// characters allowed in a key, which a brace is not.
fn scan_menu(text: &str, file: &Path, calls: &mut Calls) {
    let mut at = 0;

    while let Some(found) = text[at..].find(MENU_QUOTE) {
        let start = at + found + 1;
        let rest = &text[start..];

        if let Some(end) = rest.find('"') {
            let key = &rest[..end];

            if key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
            {
                calls.calls.push(Call {
                    file: file.to_path_buf(),
                    line: line(text, start),
                    key: key.to_owned(),
                });
            }
        }

        at = start;
    }
}

/// The dotted name the call is made on, as written before the `.`.
fn receiver(text: &str, dot: usize) -> &str {
    let start = text[..dot]
        .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
        .map_or(0, |at| at + 1);

    &text[start..dot]
}

/// The key a string literal at the front of `text` names, when one is there.
///
/// An interpolated string and a raw string are not read, since what they say is not known until
/// the program runs.
fn literal(text: &str) -> Option<String> {
    if text.starts_with("$\"") || text.starts_with("$@\"") || text.starts_with("\"\"\"") {
        return None;
    }

    if let Some(rest) = text.strip_prefix("@\"") {
        return verbatim(rest);
    }

    let mut key = String::new();
    let mut chars = text.strip_prefix('"')?.chars();

    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(key),
            '\\' => match chars.next()? {
                'n' => key.push('\n'),
                'r' => key.push('\r'),
                't' => key.push('\t'),
                other => key.push(other),
            },
            _ => key.push(c),
        }
    }

    None
}

/// The key a verbatim string names, where a doubled quote is one quote and nothing is escaped.
fn verbatim(rest: &str) -> Option<String> {
    let mut key = String::new();
    let mut chars = rest.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '"' {
            key.push(c);
        } else if chars.peek() == Some(&'"') {
            key.push('"');
            chars.next();
        } else {
            return Some(key);
        }
    }

    None
}

/// The line the byte at `at` sits on, counting from one.
fn line(text: &str, at: usize) -> usize {
    text[..at].matches('\n').count() + 1
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Calls, scan, scan_menu};

    /// The keys `source` names, and how many calls named one this cannot read.
    fn keys(source: &str) -> (Vec<String>, usize) {
        let mut calls = Calls {
            calls: Vec::new(),
            unchecked: 0,
        };

        scan(source, Path::new("test.cs"), &mut calls);

        (
            calls.calls.into_iter().map(|call| call.key).collect(),
            calls.unchecked,
        )
    }

    #[test]
    fn a_key_is_read_from_every_way_the_service_is_held() {
        let source = r#"
            var one = RolaI18N.Get("rorolala_vcs.one");
            var two = _i18n.Get("core.two", value);
            var three = _services.I18n.Get("window.three");
            var four = RolaI18N.Get(@"verbatim.key");
        "#;

        let (found, unchecked) = keys(source);

        assert_eq!(
            found,
            [
                "rorolala_vcs.one",
                "core.two",
                "window.three",
                "verbatim.key"
            ]
        );
        assert_eq!(unchecked, 0);
    }

    #[test]
    fn a_key_that_is_not_a_literal_is_counted_rather_than_guessed() {
        let (found, unchecked) =
            keys(r#"var one = _i18n.Get(label); var two = _i18n.Get($"a.{part}");"#);

        assert!(found.is_empty(), "keys were read: {found:?}");
        assert_eq!(unchecked, 2);
    }

    #[test]
    fn a_get_on_something_that_is_not_the_i18n_service_is_not_read() {
        let (found, unchecked) =
            keys(r#"var one = config.Get("not.a.key"); var two = map.Get("x");"#);

        assert!(found.is_empty(), "keys were read: {found:?}");
        assert_eq!(unchecked, 0);
    }

    /// The menu paths `source` names.
    fn menu(source: &str) -> (Vec<String>, usize) {
        let mut calls = Calls {
            calls: Vec::new(),
            unchecked: 0,
        };

        scan_menu(source, Path::new("test.cs"), &mut calls);

        (
            calls.calls.into_iter().map(|call| call.key).collect(),
            calls.unchecked,
        )
    }

    #[test]
    fn a_menu_path_is_read_from_the_literals_it_is_written_in() {
        let source = r#"
            new MenuLeaf(["menu.entry.copy", "menu.entry.file"], 10, () => { });
            var other = "menu.entry.cut";
            var own = "rorolala_vcs.own.key";
        "#;

        let (found, unchecked) = menu(source);

        assert_eq!(
            found,
            ["menu.entry.copy", "menu.entry.file", "menu.entry.cut"]
        );
        assert_eq!(unchecked, 0);
    }
}
