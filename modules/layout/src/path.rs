//! A path as a layout names it.

use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use unicode_normalization::UnicodeNormalization as _;

use crate::error::LayoutError;

/// A path a Layout binds a `Uuid` to.
///
/// It is held normalized: components joined by `/`, relative, with no empty component and no `.`
/// or `..`, and each component NFC-normalized. So one path has one spelling wherever it was
/// written, and the string itself is the key, a path is compared and stored as the bytes it
/// normalizes to.
///
/// NFC is what makes a name written with combining characters on one machine and with precomposed
/// ones on another one path rather than two. Case is kept as it was written: whether two names
/// differing only in case are the same is the machine's business, and folding it here would change
/// what a path means.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayoutPath {
    /// The whole path, components joined by `/`, as it is compared and stored.
    text: String,
}

impl LayoutPath {
    /// The path `text` names, with `/` or `\` as separators, normalized.
    ///
    /// A `.` component is dropped and a `..` climbs back over the component before it; a `..` that
    /// would climb past the root is refused rather than clamped, since a path that reaches out of
    /// the layout is not one it names.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::Path`] if it names nothing, or if a `..` climbs past the root.
    pub fn new(text: &str) -> Result<Self, LayoutError> {
        let mut components: Vec<&str> = Vec::new();

        for part in text.split(['/', '\\']) {
            match part {
                "" | "." => {}
                ".." => {
                    if components.pop().is_none() {
                        return Err(LayoutError::Path(text.to_owned()));
                    }
                }
                other => components.push(other),
            }
        }

        Self::of(text, &components)
    }

    /// The path `path` names, relative to the layout's root, normalized.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::Path`] if it is absolute, names nothing, holds a component that is
    /// not UTF-8, or climbs past the root.
    pub fn from_relative(path: &Path) -> Result<Self, LayoutError> {
        let mut components: Vec<&str> = Vec::new();

        for component in path.components() {
            match component {
                Component::Normal(name) => {
                    let Some(name) = name.to_str() else {
                        return Err(LayoutError::Path(path.display().to_string()));
                    };
                    components.push(name);
                }
                Component::CurDir => {}
                Component::ParentDir => {
                    if components.pop().is_none() {
                        return Err(LayoutError::Path(path.display().to_string()));
                    }
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(LayoutError::Path(path.display().to_string()));
                }
            }
        }

        Self::of(&path.display().to_string(), &components)
    }

    /// The normalized path `components` makes, or a refusal naming `shown`.
    fn of(shown: &str, components: &[&str]) -> Result<Self, LayoutError> {
        if components.is_empty() {
            return Err(LayoutError::Path(shown.to_owned()));
        }

        let mut text = String::new();
        for (at, component) in components.iter().enumerate() {
            if at > 0 {
                text.push('/');
            }
            text.extend(component.nfc());
        }

        Ok(Self { text })
    }

    /// The path the layout's own root is named by.
    ///
    /// It is the one path a Layout may hold that names no component: the entry at it is the root
    /// itself, which is how a tree that covers the whole Workspace or the whole Vault is recorded.
    /// Only a tree entry may be at it. It is written down as an empty string and displayed as `.`.
    #[must_use]
    pub const fn root() -> Self {
        Self {
            text: String::new(),
        }
    }

    /// The path stored as `text`: the root when it is empty, and a path otherwise.
    ///
    /// It is what a reader of a log or a snapshot uses. A command taking a name from a person uses
    /// [`new`](Self::new), which refuses a name that holds no component.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::Path`] if `text` is not the empty string and does not name a path.
    pub fn from_stored(text: &str) -> Result<Self, LayoutError> {
        if text.is_empty() {
            return Ok(Self::root());
        }

        Self::new(text)
    }

    /// Whether this path names the layout's root rather than a component.
    #[must_use]
    pub const fn is_root(&self) -> bool {
        self.text.is_empty()
    }

    /// The path as it is compared and stored: components joined by `/`, or nothing at the root.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The path as a [`PathBuf`], the way the platform writes separators.
    ///
    /// The root is the empty path, so joining it onto the root directory names the root itself.
    #[must_use]
    pub fn to_path_buf(&self) -> PathBuf {
        self.text.split('/').collect()
    }
}

impl fmt::Display for LayoutPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A person spells the root as the directory they are in.
        if self.is_root() {
            return formatter.write_str(".");
        }

        formatter.write_str(&self.text)
    }
}

impl FromStr for LayoutPath {
    type Err = LayoutError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::new(text)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::LayoutPath;

    #[test]
    fn a_path_is_written_with_one_separator_and_no_empty_components() {
        assert_eq!(LayoutPath::new("a//b/./c").unwrap().as_str(), "a/b/c");
        assert_eq!(LayoutPath::new("a\\b\\c").unwrap().as_str(), "a/b/c");
        assert_eq!(LayoutPath::new("a/b/c").unwrap().as_str(), "a/b/c");
    }

    #[test]
    fn a_dot_dot_climbs_and_one_that_escapes_is_refused() {
        assert_eq!(LayoutPath::new("a/b/../c").unwrap().as_str(), "a/c");
        assert!(LayoutPath::new("a/../..").is_err());
        assert!(LayoutPath::new("..").is_err());
    }

    #[test]
    fn a_path_that_names_nothing_is_refused() {
        // The root is built rather than parsed into, so a name from a person still names a place.
        assert!(LayoutPath::new("").is_err());
        assert!(LayoutPath::new(".").is_err());
        assert!(LayoutPath::new("/").is_err());
    }

    #[test]
    fn an_absolute_path_or_a_drive_is_refused() {
        assert!(LayoutPath::from_relative(Path::new("/etc/passwd")).is_err());
        assert!(LayoutPath::from_relative(Path::new("a/b")).is_ok());
    }

    #[test]
    fn the_root_is_the_empty_path_that_displays_as_the_directory_itself() {
        let root = LayoutPath::root();

        assert!(root.is_root());
        assert_eq!(root.as_str(), "");
        assert_eq!(root.to_string(), ".");
        assert_eq!(root.to_path_buf(), Path::new(""));
        assert_eq!(LayoutPath::from_stored("").unwrap(), root);
    }

    #[test]
    fn a_stored_path_reads_back_as_one() {
        assert_eq!(LayoutPath::from_stored("a/b").unwrap().as_str(), "a/b");
        assert!(LayoutPath::from_stored("../b").is_err());
    }

    #[test]
    fn a_name_is_normalized_to_one_spelling() {
        // U+00E9 and U+0065 U+0301 are the same name written two ways.
        let composed = LayoutPath::new("caf\u{e9}").unwrap();
        let decomposed = LayoutPath::new("cafe\u{301}").unwrap();

        assert_eq!(composed, decomposed);
        assert_eq!(composed.as_str(), "caf\u{e9}");
    }
}
