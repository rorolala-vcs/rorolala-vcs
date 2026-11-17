//! Writing one object into a VCS index.

use std::path::Path;
use std::str::FromStr;

use rorolala_storage::Key;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vcs::{Creator, Message, VCSIndex, Variant, Version};
use serde::Deserialize;

use crate::write::WriteError;

/// One object, as a request document names it.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RawObject {
    /// A variant: the stored entry it points at, its base, and what made it.
    Variant {
        /// The content key the variant points at.
        storage: String,
        /// The base version's content key.
        base_version: String,
        /// The variant it merges in, when it merges one.
        join: Option<String>,
        /// The creator's content key.
        creator: String,
        /// The message's content key.
        message: String,
    },
    /// A version: the variant it points at.
    Version {
        /// The variant's content key.
        variant: String,
    },
    /// A creator: the name it is.
    Creator {
        /// The name.
        text: String,
    },
    /// A message: the text it is.
    Message {
        /// The text.
        text: String,
    },
}

/// Writes one object into the index a directory is in.
///
/// # FFI
///
/// An owned string the caller releases with `free_string`, the key the object was written under,
/// or a `RolaWriteError *` the caller releases with `free_rola_write_error`. The request is one
/// object, `{"kind":"variant",...}`, `{"kind":"version",...}`, `{"kind":"creator",...}` or
/// `{"kind":"message",...}`.
///
/// # Errors
///
/// Returns [`WriteError::NoIndex`] when no index is there, [`WriteError::Json`] when the request
/// does not read, [`WriteError::Precondition`] when a key or a text is not one the index accepts,
/// and [`WriteError::Index`] when the index refuses the write.
#[lazyffi(export = rola_write_index_object_json)]
pub fn write_object_json(directory: &Path, request: &str) -> Result<String, WriteError> {
    let index = VCSIndex::locate(directory).ok_or(WriteError::NoIndex)?;
    let raw: RawObject = serde_json::from_str(request).map_err(|error| WriteError::Json {
        reason: format!("{error:?}"),
    })?;

    match raw {
        RawObject::Variant {
            storage,
            base_version,
            join,
            creator,
            message,
        } => {
            let variant = Variant::new_bare_variant(
                digest(&storage, "storage")?,
                digest(&base_version, "base_version")?,
                join.as_deref()
                    .map(|join| digest(join, "join"))
                    .transpose()?,
                digest(&creator, "creator")?,
                digest(&message, "message")?,
                0,
            );
            index
                .write_variant(&variant)
                .map_err(|error| WriteError::Index {
                    reason: format!("{error:?}"),
                })
        }
        RawObject::Version { variant } => {
            let version = Version::new_bare_version(digest(&variant, "variant")?, 0);
            index
                .write_version(&version)
                .map_err(|error| WriteError::Index {
                    reason: format!("{error:?}"),
                })
        }
        RawObject::Creator { text } => {
            let creator = Creator::try_from(text).map_err(|error| WriteError::Precondition {
                what: "creator".to_owned(),
                reason: format!("{error:?}"),
            })?;
            index
                .write_creator(&creator)
                .map_err(|error| WriteError::Index {
                    reason: format!("{error:?}"),
                })
        }
        RawObject::Message { text } => {
            let message = Message::try_from(text).map_err(|error| WriteError::Precondition {
                what: "message".to_owned(),
                reason: format!("{error:?}"),
            })?;
            index
                .write_message(&message)
                .map_err(|error| WriteError::Index {
                    reason: format!("{error:?}"),
                })
        }
    }
}

/// The digest a request's key names.
fn digest(key: &str, what: &str) -> Result<[u8; 32], WriteError> {
    let key = Key::from_str(key).map_err(|error| WriteError::Precondition {
        what: what.to_owned(),
        reason: format!("{error:?}"),
    })?;

    Ok(*key.digest())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_utils_location::Locate as _;
    use rorolala_vcs::VCSIndex;

    use super::write_object_json;

    #[test]
    fn a_creator_and_a_message_are_written() {
        let dir = std::env::temp_dir().join(format!("rola-write-index-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let _ = rorolala_workspace::Workspace::create(&dir);
        let index = VCSIndex::locate(&dir).unwrap();

        let creator = write_object_json(&dir, r#"{"kind":"creator","text":"alice"}"#).unwrap();
        assert!(index.object_json(&creator).unwrap().contains("\"alice\""));

        let message = write_object_json(&dir, r#"{"kind":"message","text":"first"}"#).unwrap();
        assert!(index.object_json(&message).unwrap().contains("\"first\""));

        let _ = fs::remove_dir_all(&dir);
    }
}
