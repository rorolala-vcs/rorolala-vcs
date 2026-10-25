//! The version a manifest is set to, checked before it is written.
//!
//! What goes into the manifest is the same string that names the changelog's section, so it has to
//! be one line and one token before anything is touched. Cargo accepts a `SemVer` version; this
//! the same three numbers and the optional pre-release and build suffixes, and refuses what it
//! cannot state in the manifest.

use crate::Error;

/// A version this program will write into the workspace manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Version(String);

impl Version {
    /// Reads `value` as the version it writes, or says why it will not.
    ///
    /// # Errors
    ///
    /// Reports [`Error::InvalidVersion`] for anything that is not three dot-separated numbers with
    /// an optional `-pre.release` and `+build` suffix.
    pub fn parse(value: &str) -> Result<Self, Error> {
        let refuse = |reason: &'static str| Error::InvalidVersion {
            value: value.to_owned(),
            reason,
        };

        if value.is_empty() {
            return Err(refuse("it is empty"));
        }

        if value
            .chars()
            .any(|character| character.is_whitespace() || matches!(character, '"' | '\'' | '\\'))
        {
            return Err(refuse("it has whitespace, a quote or a backslash"));
        }

        let (core, suffix) = value
            .split_once(['-', '+'])
            .map_or((value, None), |(core, rest)| (core, Some(rest)));

        let parts: Vec<&str> = core.split('.').collect();
        if parts.len() != 3 {
            return Err(refuse("it is not MAJOR.MINOR.PATCH"));
        }

        for part in &parts {
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(refuse("a component is not a number"));
            }

            if part.len() > 1 && part.starts_with('0') {
                return Err(refuse("a component has a leading zero"));
            }
        }

        if let Some(suffix) = suffix {
            if suffix.is_empty() {
                return Err(refuse("it has a separator but no suffix"));
            }

            if !suffix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
            {
                return Err(refuse("its suffix has a character a version cannot carry"));
            }
        }

        Ok(Self(value.to_owned()))
    }

    /// The version as it is written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::Version;

    #[test]
    fn three_numbers_are_a_version() {
        for value in ["0.1.0", "1.2.3", "10.20.30", "0.0.0"] {
            assert_eq!(Version::parse(value).unwrap().as_str(), value);
        }
    }

    #[test]
    fn a_pre_release_and_a_build_suffix_are_part_of_the_version() {
        for value in [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0+build.7",
            "1.0.0-rc.1+build",
        ] {
            assert_eq!(Version::parse(value).unwrap().as_str(), value);
        }
    }

    #[test]
    fn what_is_not_three_numbers_is_refused() {
        for value in [
            "", "1", "1.2", "1.2.3.4", "1..3", "a.b.c", "1.2.x", "01.2.3",
        ] {
            assert!(Version::parse(value).is_err(), "`{value}` was accepted");
        }
    }

    #[test]
    fn what_cannot_be_written_into_the_manifest_is_refused() {
        for value in [
            "1.2.3 ",
            "1.2.3\n",
            "1.2.3\"",
            "1.2.3\\",
            "1.2.3-",
            "1.2.3-rc_1",
        ] {
            assert!(Version::parse(value).is_err(), "`{value}` was accepted");
        }
    }
}
