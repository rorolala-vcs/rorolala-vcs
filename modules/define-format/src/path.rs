//! A path a lock places a material at.

use std::fmt;
use std::str::FromStr;

use unicode_normalization::UnicodeNormalization as _;

use crate::error::{FormatError, FormatErrorKind};

/// A path a material is placed at, relative to the project's root.
///
/// It is held normalized: components joined by `/`, no leading separator, each component
/// NFC-normalized, so one place has one spelling wherever it was written. A lock's line writes it
/// with a leading `/`; what is held is the path without it.
///
/// What a path may not name is anything outside the project or about a workspace's own data: a
/// drive letter, an empty component, a component that climbs with `..`, and a path at the top of
/// the `.rola/` directory.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LockPath {
    /// The path as it is compared and written: components joined by `/`.
    text: String,
}

impl LockPath {
    /// Reads `text` as a path: an optional leading `/` or `\`, then `/`-separated components.
    ///
    /// # Errors
    ///
    /// Returns [`FormatErrorKind::InvalidPath`] when what is given is empty, has an empty
    /// component, carries `.` or `..`, names `.rola/` at the top, holds a backslash after the
    /// leading separator, a control character, a component with whitespace at either end, or a
    /// first component ending with `:` (a drive letter).
    pub fn new(text: &str) -> Result<Self, FormatError> {
        let refused = || {
            FormatError::plain(FormatErrorKind::InvalidPath {
                text: text.to_owned(),
            })
        };

        // The one leading separator is the root marker a line begins with; the rest is the path.
        let rest = text
            .strip_prefix('/')
            .or_else(|| text.strip_prefix('\\'))
            .unwrap_or(text);

        if rest.is_empty() {
            return Err(refused());
        }

        let mut components: Vec<String> = Vec::new();

        for (at, part) in rest.split('/').enumerate() {
            if part.is_empty() || part == "." || part == ".." {
                return Err(refused());
            }

            // A path is one word of a line, so a component that begins or ends with whitespace
            // could not be read back: what a line trims away is gone for good.
            if part.trim() != part {
                return Err(refused());
            }

            // A backslash is a separator on one platform and nothing on another, and a lock line
            // has no escaping to say which was meant.
            if part.contains('\\') || part.chars().any(char::is_control) {
                return Err(refused());
            }

            if at == 0 && (part.ends_with(':') || part == ".rola") {
                return Err(refused());
            }

            components.push(part.nfc().collect());
        }

        Ok(Self {
            text: components.join("/"),
        })
    }

    /// The path as it is compared and written: components joined by `/`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for LockPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text)
    }
}

impl FromStr for LockPath {
    type Err = FormatError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::new(text)
    }
}

#[cfg(test)]
mod tests {
    use super::LockPath;
    use crate::error::FormatErrorKind;

    /// Whether `text` is refused as a path, without saying why.
    fn refused(text: &str) -> bool {
        LockPath::new(text).is_err()
    }

    #[test]
    fn a_leading_separator_is_the_root_marker_and_is_not_held() {
        assert_eq!(
            LockPath::new("/Assets/hero.psd").unwrap().as_str(),
            "Assets/hero.psd"
        );
        assert_eq!(
            LockPath::new("\\Assets/hero.psd").unwrap().as_str(),
            "Assets/hero.psd"
        );
        assert_eq!(
            LockPath::new("Assets/hero.psd").unwrap().as_str(),
            "Assets/hero.psd"
        );
    }

    #[test]
    fn a_path_with_a_space_in_a_component_is_read_as_written() {
        assert_eq!(
            LockPath::new("/Program Files/Some Game/Data.pak")
                .unwrap()
                .as_str(),
            "Program Files/Some Game/Data.pak"
        );
    }

    #[test]
    fn a_name_is_normalized_to_one_spelling() {
        // U+00E9 and U+0065 U+0301 are the same name written two ways.
        assert_eq!(
            LockPath::new("/caf\u{e9}/x").unwrap(),
            LockPath::new("/cafe\u{301}/x").unwrap()
        );
    }

    #[test]
    fn what_cannot_be_one_place_in_the_project_is_refused() {
        for text in [
            "",               // nothing at all
            "/",              // only the root marker
            "/a//b",          // an empty component
            "/a/",            // a trailing separator
            "/a/./b",         // this directory
            "/a/../b",        // climbing
            "/..",            // climbing past the root
            ".",              // this directory
            "/C:/Users/x",    // a drive letter
            "/.rola/storage", // a workspace's own data
            "/a/\\b",         // a backslash after the separator
            "/a\u{0}b",       // a control character
            "/a /b",          // a component that begins with whitespace
            "/a/ b",          // a component that ends with whitespace
            "//server/share", // a network share, which begins with an empty component
        ] {
            let error = LockPath::new(text).unwrap_err();
            assert!(
                matches!(error.kind(), FormatErrorKind::InvalidPath { .. }),
                "{text}"
            );
            assert!(refused(text), "{text}");
        }

        // A directory named `.rola` below the top is a path, not the workspace's own data.
        assert!(LockPath::new("/sub/.rola/x").is_ok());
    }
}
