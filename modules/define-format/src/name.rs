//! The name a lock gives one of its Vaults.

use std::fmt;

use crate::error::{FormatError, FormatErrorKind};

/// The longest a Vault name may be, in bytes.
const LONGEST_NAME: usize = 64;

/// The name a lock gives one of the Vaults its content can be fetched from.
///
/// A name is ASCII letters, digits, `-` and `_`: one word of a line, and a key a `rola.toml` can
/// state without quoting. `latest` is refused, because it is what a mapping's pin means when it
/// names no source.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VaultName {
    /// The name as it is written.
    text: String,
}

impl VaultName {
    /// Reads `text` as a Vault's name.
    ///
    /// # Errors
    ///
    /// Returns [`FormatErrorKind::InvalidName`] when what is given is empty, longer than 64 bytes,
    /// not ASCII letters, digits, `-` or `_`, or is `latest`.
    pub fn new(text: &str) -> Result<Self, FormatError> {
        let refused = || {
            FormatError::plain(FormatErrorKind::InvalidName {
                name: text.to_owned(),
            })
        };

        if text.is_empty() || text.len() > LONGEST_NAME || text == "latest" {
            return Err(refused());
        }

        if !text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(refused());
        }

        Ok(Self {
            text: text.to_owned(),
        })
    }

    /// The name as it is written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for VaultName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::VaultName;
    use crate::error::FormatErrorKind;

    #[test]
    fn a_name_is_one_ascii_word() {
        for name in ["origin", "origin-2", "mirror_backup", "Vault1"] {
            assert_eq!(VaultName::new(name).unwrap().as_str(), name);
        }
    }

    #[test]
    fn what_is_not_one_ascii_word_or_is_reserved_is_refused() {
        for name in [
            "",              // nothing at all
            "latest",        // what a pin means when it names no source
            "a b",           // a space
            "a/b",           // a separator
            "a.b",           // a character a name does not carry
            "caf\u{e9}",     // not ASCII
            &"a".repeat(65), // too long
        ] {
            let error = VaultName::new(name).unwrap_err();
            assert!(
                matches!(error.kind(), FormatErrorKind::InvalidName { .. }),
                "{name}"
            );
        }
    }
}
