//! The draft a person fills in before `write`.
//!
//! A draft is front matter and a body, like a skill: the front matter says what the entry is, and
//! the body is what it will carry under its title. It is read as YAML rather than as lines so a
//! quoted summary, a folded string or a single scope written without brackets is read as what it
//! says.

use std::path::Path;

use serde::Deserialize;

use crate::Error;
use crate::document::{Category, Entry};

/// Reads the draft at `path` as the category and entry `write` merges.
///
/// # Errors
///
/// Reports [`Error::Io`] when the draft cannot be read, [`Error::Draft`] when it is not front matter
/// and a body or its fields are not what they must be, and [`Error::InvalidCategory`] or
/// [`Error::InvalidScope`] for the values it names.
pub fn read(path: &Path) -> Result<(Category, Entry), Error> {
    let text = std::fs::read_to_string(path).map_err(|error| Error::Io {
        doing: "reading the draft",
        detail: format!("{}: {error}", path.display()),
    })?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);

    let (front, body) = split(text).map_err(|detail| draft_error(path, detail))?;
    let front: DraftFront =
        serde_yaml::from_str(&front).map_err(|error| draft_error(path, error.to_string()))?;

    let summary = front
        .summary
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if summary.is_empty() {
        return Err(draft_error(
            path,
            "`summary` is empty; write what changed in one line",
        ));
    }

    let category = front
        .category
        .ok_or_else(|| draft_error(path, "`category` is missing"))?;
    let category = Category::from_draft(&category)?;

    let scopes: Vec<String> = front
        .scope
        .map(ScopeValue::into_vec)
        .unwrap_or_default()
        .into_iter()
        .map(|scope| scope.trim().to_owned())
        .collect();

    for scope in &scopes {
        validate_scope(scope)?;
    }

    Ok((
        category,
        Entry {
            breaking: front.breaking,
            scopes,
            summary,
            body: body
                .trim()
                .lines()
                .map(|line| line.trim_end().to_owned())
                .collect(),
            heading_level: 0,
            number: 0,
        },
    ))
}

/// The front matter the draft carries.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct DraftFront {
    /// The one-line summary the entry is titled with.
    #[serde(default)]
    summary: Option<String>,
    /// The category the entry belongs to.
    #[serde(default)]
    category: Option<String>,
    /// The scopes the change touches, as a list or as one string.
    #[serde(default)]
    scope: Option<ScopeValue>,
    /// Whether the change is incompatible with what callers had.
    #[serde(default)]
    breaking: bool,
}

/// The `scope` field: one scope or several.
#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum ScopeValue {
    /// One scope written as a string.
    One(String),
    /// Several scopes written as a list.
    Many(Vec<String>),
}

impl ScopeValue {
    /// The scopes as a list, whichever way they were written.
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(scope) => vec![scope],
            Self::Many(scopes) => scopes,
        }
    }
}

/// The front matter and the body of `text`.
fn split(text: &str) -> Result<(String, String), &'static str> {
    let mut lines = text.lines();
    let first = lines.next().unwrap_or_default().trim();

    if first != "---" {
        return Err("the draft must start with a `---` front matter line");
    }

    let mut front = String::new();
    let mut body = String::new();
    let mut in_front = true;

    for line in lines {
        if in_front {
            if line.trim() == "---" {
                in_front = false;
                continue;
            }

            front.push_str(line);
            front.push('\n');
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }

    if in_front {
        return Err("the draft's front matter is not closed by a `---` line");
    }

    Ok((front, body))
}

/// Whether `scope` can be written into an entry title.
fn validate_scope(scope: &str) -> Result<(), Error> {
    if scope.is_empty()
        || scope
            .chars()
            .any(|character| matches!(character, '[' | ']' | '`') || character.is_whitespace())
    {
        return Err(Error::InvalidScope {
            value: scope.to_owned(),
        });
    }

    Ok(())
}

/// The failure of a draft that is not the draft it is named for.
fn draft_error(path: &Path, detail: impl Into<String>) -> Error {
    Error::Draft {
        path: path.to_path_buf(),
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    /// A draft written to a temporary file, and its path.
    fn written(name: &str, text: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("rola-changelog-{name}-{}.md", std::process::id()));
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn a_draft_states_its_category_scopes_and_breaking_field() {
        let path = written(
            "stated",
            "\
---
summary: Fix the blocking read
category: bugfix
scope: [\"cli\", \"rola\"]
breaking: true
---

The body.
",
        );

        let (category, entry) = super::read(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(category.heading(), "Bugfix");
        assert!(entry.breaking);
        assert_eq!(entry.scopes.join(","), "cli,rola");
        assert_eq!(entry.summary, "Fix the blocking read");
        assert_eq!(entry.body.join("\n"), "The body.");
    }

    #[test]
    fn a_scope_may_be_one_string_or_left_out() {
        let one_path = written(
            "one",
            "---\nsummary: S\ncategory: feature\nscope: cli\n---\n",
        );
        let none_path = written("none", "---\nsummary: S\ncategory: feature\n---\n");

        let (_, one) = super::read(&one_path).unwrap();
        let (_, none) = super::read(&none_path).unwrap();
        std::fs::remove_file(&one_path).ok();
        std::fs::remove_file(&none_path).ok();

        assert_eq!(one.scopes.join(","), "cli");
        assert_eq!(none.scopes, Vec::<String>::new());
    }

    #[test]
    fn a_summary_is_one_line() {
        let path = written(
            "folded",
            "---\nsummary: first\n  second\ncategory: bugfix\n---\n",
        );

        let (_, entry) = super::read(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(entry.summary, "first second");
    }

    #[test]
    fn what_is_not_a_draft_is_refused() {
        for (name, text) in [
            ("empty", ""),
            ("bare", "no front matter\n"),
            ("unclosed", "---\nsummary: S\ncategory: bugfix\n"),
            ("summary", "---\ncategory: bugfix\n---\n"),
            ("category", "---\nsummary: S\ncategory: nope\n---\n"),
            (
                "scope",
                "---\nsummary: S\ncategory: bugfix\nscope: [\"a b\"]\n---\n",
            ),
        ] {
            let path = written(name, text);
            assert!(super::read(&path).is_err(), "`{text}` was accepted");
            std::fs::remove_file(&path).ok();
        }
    }

    #[test]
    fn an_unknown_field_is_refused_rather_than_ignored() {
        let path = written(
            "unknown",
            "---\nsummary: S\ncategory: bugfix\nbreking: true\n---\n",
        );

        assert!(super::read(&path).is_err());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn the_breaking_field_marks_a_change() {
        let path = written(
            "flag",
            "---\nsummary: S\ncategory: optimized\nbreaking: true\n---\n",
        );

        let (category, entry) = super::read(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(category.heading(), "Optimized");
        assert!(entry.breaking);
    }

    #[test]
    fn breaking_is_not_a_category() {
        let path = written(
            "breaking-category",
            "---\nsummary: S\ncategory: breaking-bugfix\nbreaking: true\n---\n",
        );

        assert!(super::read(&path).is_err());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_draft_that_is_not_there_is_an_io_failure() {
        let path = Path::new("__a_draft_that_is_not_there.md");
        assert!(super::read(path).is_err());
    }
}
