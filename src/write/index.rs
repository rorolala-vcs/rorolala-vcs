//! Writing one object into a VCS index.

use std::path::Path;
use std::str::FromStr;

use rorolala_storage::Key;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vcs::{Creator, Message, VCSIndex, VCSWrite as _, Variant, Version};
use serde::{Deserialize, Serialize};

use crate::write::WriteError;

/// The variant and version a file's content was recorded as.
#[derive(Debug, Clone, Serialize)]
pub struct FileVersion {
    /// The variant the version points at.
    pub variant: Key,
    /// The version the Layout records.
    pub version: Key,
}

/// Records a file's content as a variant and a version.
///
/// The creator and the message are written first, because the variant names them. `base` is the
/// variant the file was on, or the root's zero hash for a first version; `join` is the variant a
/// merge brings in.
///
/// # Errors
///
/// Returns [`WriteError::Precondition`] when the creator or the message is too long, and
/// [`WriteError::Index`] when the index refuses a write.
pub fn write_file_version(
    index: &VCSIndex,
    storage: &Key,
    base: &[u8; 32],
    join: Option<&[u8; 32]>,
    creator_name: &str,
    message_text: &str,
) -> Result<FileVersion, WriteError> {
    let creator =
        Creator::try_from(creator_name.to_owned()).map_err(|error| WriteError::Precondition {
            what: "creator".to_owned(),
            reason: error.to_string(),
        })?;
    let message =
        Message::try_from(message_text.to_owned()).map_err(|error| WriteError::Precondition {
            what: "message".to_owned(),
            reason: error.to_string(),
        })?;
    let creator_key = *creator.hash().digest();
    let message_key = *message.hash().digest();
    index
        .write_creator(&creator)
        .map_err(|error| WriteError::Index {
            reason: format!("{error:?}"),
        })?;
    index
        .write_message(&message)
        .map_err(|error| WriteError::Index {
            reason: format!("{error:?}"),
        })?;

    let variant = Variant::new_bare_variant(
        *storage.digest(),
        *base,
        join.copied(),
        creator_key,
        message_key,
        0,
    );
    let variant_key = *variant.hash().digest();
    index
        .write_variant(&variant)
        .map_err(|error| WriteError::Index {
            reason: format!("{error:?}"),
        })?;

    let version = Version::new_bare_version(variant_key, 0);
    let version_key = *version.hash().digest();
    index
        .write_version(&version)
        .map_err(|error| WriteError::Index {
            reason: format!("{error:?}"),
        })?;

    Ok(FileVersion {
        variant: Key::new(variant_key),
        version: Key::new(version_key),
    })
}

/// Records a file's content as a variant and a version in the index a directory is in.
///
/// # FFI
///
/// An owned JSON string the caller releases with `free_string`,
/// `{"variant":"blake3:...","version":"blake3:..."}`, or a `RolaWriteError *` the caller releases
/// with `free_rola_write_error`. `join` is the empty string when the variant merges nothing.
///
/// # Errors
///
/// Returns [`WriteError::NoIndex`] when no index is there, [`WriteError::Precondition`] when a key
/// or a text does not read, and [`WriteError::Index`] when the index refuses a write.
#[lazyffi(export = rola_write_index_file_version)]
pub fn write_file_version_at(
    directory: &Path,
    storage: &str,
    base: &str,
    join: &str,
    creator: &str,
    message: &str,
) -> Result<String, WriteError> {
    let index = VCSIndex::locate(directory).ok_or(WriteError::NoIndex)?;
    let storage = digest(storage, "storage")?;
    let base = digest(base, "base")?;
    let join = (!join.is_empty())
        .then(|| digest(join, "join"))
        .transpose()?;

    let written = write_file_version(
        &index,
        &Key::new(storage),
        &base,
        join.as_ref(),
        creator,
        message,
    )?;

    serde_json::to_string(&written).map_err(|error| WriteError::Json {
        reason: error.to_string(),
    })
}

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

    use super::{write_file_version, write_object_json};

    #[test]
    fn a_file_version_names_its_variant_and_creator() {
        let dir = std::env::temp_dir().join(format!("rola-write-version-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let _ = rorolala_workspace::Workspace::create(&dir);
        let index = VCSIndex::locate(&dir).unwrap();
        let store = rorolala_storage::RorolalaStorage::create(dir.join(".rola/storage"));
        let content = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(store.write_object(b"content", rorolala_storage::Codec::Raw))
            .unwrap();

        let written =
            write_file_version(&index, &content, &[0; 32], None, "alice", "first").unwrap();

        assert_ne!(written.version.to_string(), "");
        assert!(
            index
                .object_json(&written.version.to_string())
                .unwrap()
                .contains("\"kind\":\"version\"")
        );

        let _ = fs::remove_dir_all(&dir);
    }

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
