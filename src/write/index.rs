//! Writing one object into a VCS index.

use std::path::Path;
use std::str::FromStr;

use rorolala_storage::Key;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vcs::{Creator, Message, ROOT_VERSION, VCSIndex, VCSWrite as _, Variant, Version};

use crate::write::WriteError;

/// The variant and version a file's content was recorded as.
#[derive(Debug, Clone)]
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
    base_num: u64,
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
    let (base, base_num) = if *base == [0; 32] {
        let root = Version::root();
        index
            .write_version(&root)
            .map_err(|error| WriteError::Index {
                reason: format!("{error:?}"),
            })?;

        (*root.hash().digest(), ROOT_VERSION)
    } else {
        (*base, base_num)
    };
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
        base,
        join.copied(),
        creator_key,
        message_key,
        base_num,
    );
    let variant_key = *variant.hash().digest();
    index
        .write_variant(&variant)
        .map_err(|error| WriteError::Index {
            reason: format!("{error:?}"),
        })?;

    let version = Version::new_bare_version(variant_key, base_num.wrapping_add(1));
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
/// A `RolaWriteFileVersion *` the caller releases with `free_rola_write_file_version`, or a
/// `RolaWriteError *` the caller releases with `free_rola_write_error`. `join` is the empty string
/// when the variant merges nothing.
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
) -> Result<WriteFileVersion, WriteError> {
    let index = VCSIndex::locate(directory).ok_or(WriteError::NoIndex)?;
    let storage = digest(storage, "storage")?;
    let base = digest(base, "base")?;
    let join = (!join.is_empty())
        .then(|| digest(join, "join"))
        .transpose()?;

    let base_num = if base == [0; 32] {
        ROOT_VERSION
    } else {
        let runtime = tokio::runtime::Runtime::new().map_err(|error| WriteError::Internal {
            reason: error.to_string(),
        })?;
        let version = index
            .read_version(&Key::new(base).to_string())
            .map_err(|error| WriteError::Index {
                reason: format!("{error:?}"),
            })?;
        runtime
            .block_on(index.version_num(&version))
            .map_err(|error| WriteError::Index {
                reason: format!("{error:?}"),
            })?
    };
    let written = write_file_version(
        &index,
        &Key::new(storage),
        &base,
        base_num,
        join.as_ref(),
        creator,
        message,
    )?;

    Ok(WriteFileVersion {
        variant: written.variant.to_string(),
        version: written.version.to_string(),
    })
}

/// The variant and version a file's content was recorded as, as the C ABI hands them over.
///
/// # FFI
///
/// A `RolaWriteFileVersion *` the caller releases with `free_rola_write_file_version`. `_variant`
/// and `_version` each return a `char *` the caller releases with `free_string`.
#[lazyffi(export = RolaWriteFileVersion)]
#[derive(Debug, Clone, Default)]
pub struct WriteFileVersion {
    /// The variant the version points at.
    variant: String,
    /// The version the Layout records.
    version: String,
}

#[lazyffi(export = rola_)]
impl WriteFileVersion {
    /// The variant the version points at.
    #[must_use]
    pub fn variant(&self) -> String {
        self.variant.clone()
    }

    /// The version the Layout records.
    #[must_use]
    pub fn version(&self) -> String {
        self.version.clone()
    }
}

/// One object, as a C caller names it.
///
/// It mirrors the request document the command line reads, except that a name the object does not
/// use is the empty string rather than absent.
///
/// # FFI
///
/// A `RolaWriteIndexObjectRequest` the caller fills: `tag` selects the object and the payload
/// companion it names holds that object's strings. Every `const char *` is borrowed for the call and
/// released by the caller, so nothing here is freed when the call returns.
#[lazyffi(export = RolaWriteIndexObjectRequest)]
#[derive(Debug, Clone)]
pub enum IndexObjectInput {
    /// A variant: the stored entry it points at, its base, and what made it.
    Variant {
        /// The content key the variant points at.
        storage: String,
        /// The base version's content key.
        base_version: String,
        /// The variant it merges in, empty when it merges none.
        join: String,
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

/// The key an index object was written under.
///
/// # FFI
///
/// A `RolaWriteIndexObject *` the caller releases with `free_rola_write_index_object`. `_key`
/// returns a `char *` the caller releases with `free_string`.
#[lazyffi(export = RolaWriteIndexObject)]
#[derive(Debug, Clone, Default)]
pub struct WriteIndexObject {
    /// The key the object was written under.
    key: String,
}

#[lazyffi(export = rola_)]
impl WriteIndexObject {
    /// The key the object was written under.
    #[must_use]
    pub fn key(&self) -> String {
        self.key.clone()
    }
}

/// Writes one object into the index a directory is in.
///
/// # FFI
///
/// `directory` is borrowed for the call, and `request` is a `RolaWriteIndexObjectRequest` the
/// caller fills and keeps, with every `const char *` in it borrowed for the call. A
/// `RolaWriteIndexObject *` the caller releases with `free_rola_write_index_object` answers it,
/// whose `_key` is the key the object was written under, or a `RolaWriteError *` the caller
/// releases with `free_rola_write_error`.
///
/// # Errors
///
/// Returns [`WriteError::NoIndex`] when no index is there, [`WriteError::Precondition`] when a key
/// or a text is not one the index accepts, and [`WriteError::Index`] when the index refuses the
/// write.
#[lazyffi(export = rola_write_index_object)]
#[allow(
    clippy::needless_pass_by_value,
    reason = "a tagged union crosses as a value, since a C caller fills the tag and the payload together"
)]
pub fn write_object(
    directory: &Path,
    request: IndexObjectInput,
) -> Result<WriteIndexObject, WriteError> {
    let index = VCSIndex::locate(directory).ok_or(WriteError::NoIndex)?;

    let key = match &request {
        IndexObjectInput::Variant {
            storage,
            base_version,
            join,
            creator,
            message,
        } => {
            let variant = Variant::new_bare_variant(
                digest(storage, "storage")?,
                digest(base_version, "base_version")?,
                (!join.is_empty())
                    .then(|| digest(join, "join"))
                    .transpose()?,
                digest(creator, "creator")?,
                digest(message, "message")?,
                0,
            );
            index
                .write_variant(&variant)
                .map_err(|error| WriteError::Index {
                    reason: format!("{error:?}"),
                })?
        }
        IndexObjectInput::Version { variant } => {
            let version = Version::new_bare_version(digest(variant, "variant")?, 0);
            index
                .write_version(&version)
                .map_err(|error| WriteError::Index {
                    reason: format!("{error:?}"),
                })?
        }
        IndexObjectInput::Creator { text } => {
            let creator =
                Creator::try_from(text.clone()).map_err(|error| WriteError::Precondition {
                    what: "creator".to_owned(),
                    reason: format!("{error:?}"),
                })?;
            index
                .write_creator(&creator)
                .map_err(|error| WriteError::Index {
                    reason: format!("{error:?}"),
                })?
        }
        IndexObjectInput::Message { text } => {
            let message =
                Message::try_from(text.clone()).map_err(|error| WriteError::Precondition {
                    what: "message".to_owned(),
                    reason: format!("{error:?}"),
                })?;
            index
                .write_message(&message)
                .map_err(|error| WriteError::Index {
                    reason: format!("{error:?}"),
                })?
        }
    };

    Ok(WriteIndexObject { key })
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
    use rorolala_vcs::view::ReadObjectView;
    use rorolala_vcs::{ROOT_VERSION, VCSIndex};

    use super::{IndexObjectInput, write_file_version, write_file_version_at, write_object};
    use crate::write::WriteError;

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

        let written = write_file_version(
            &index,
            &content,
            &[0; 32],
            ROOT_VERSION,
            None,
            "alice",
            "first",
        )
        .unwrap();

        assert_ne!(written.version.to_string(), "");
        assert!(matches!(
            index.object_view(&written.version.to_string()).unwrap(),
            ReadObjectView::Version { .. }
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_creator_and_a_message_are_written() {
        let dir = std::env::temp_dir().join(format!("rola-write-index-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let _ = rorolala_workspace::Workspace::create(&dir);
        let index = VCSIndex::locate(&dir).unwrap();

        let creator = write_object(
            &dir,
            IndexObjectInput::Creator {
                text: "alice".to_owned(),
            },
        )
        .unwrap()
        .key();
        assert!(matches!(
            index.object_view(&creator).unwrap(),
            ReadObjectView::Creator { text } if text == "alice"
        ));

        let message = write_object(
            &dir,
            IndexObjectInput::Message {
                text: "first".to_owned(),
            },
        )
        .unwrap()
        .key();
        assert!(matches!(
            index.object_view(&message).unwrap(),
            ReadObjectView::Message { text } if text == "first"
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_version_at_a_directory_reads_its_base_from_the_index() {
        let dir = std::env::temp_dir().join(format!("rola-write-at-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let _ = rorolala_workspace::Workspace::create(&dir);
        let storage = rorolala_storage::Key::new([4; 32]).to_string();
        let zero = rorolala_storage::Key::new([0; 32]).to_string();

        let first = write_file_version_at(&dir, &storage, &zero, "", "alice", "first").unwrap();
        assert_ne!(first.variant(), "");
        assert_ne!(first.version(), "");

        let join = rorolala_storage::Key::new([5; 32]).to_string();
        let second =
            write_file_version_at(&dir, &storage, &first.version(), &join, "alice", "second")
                .unwrap();
        assert_ne!(second.version(), first.version());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_key_that_does_not_read_is_a_precondition() {
        let dir = std::env::temp_dir().join(format!("rola-write-bad-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let _ = rorolala_workspace::Workspace::create(&dir);
        let zero = rorolala_storage::Key::new([0; 32]).to_string();

        assert!(matches!(
            write_file_version_at(&dir, "not a key", &zero, "", "alice", "first"),
            Err(WriteError::Precondition { .. })
        ));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_variant_and_a_version_object_are_written_by_key() {
        let dir = std::env::temp_dir().join(format!("rola-write-object-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let _ = rorolala_workspace::Workspace::create(&dir);
        let index = VCSIndex::locate(&dir).unwrap();
        let key = rorolala_storage::Key::new;

        let variant = write_object(
            &dir,
            IndexObjectInput::Variant {
                storage: key([1; 32]).to_string(),
                base_version: key([0; 32]).to_string(),
                join: key([2; 32]).to_string(),
                creator: key([3; 32]).to_string(),
                message: key([4; 32]).to_string(),
            },
        )
        .unwrap()
        .key();
        assert!(matches!(
            index.object_view(&variant).unwrap(),
            ReadObjectView::Variant { .. }
        ));

        let version = write_object(&dir, IndexObjectInput::Version { variant })
            .unwrap()
            .key();
        assert!(matches!(
            index.object_view(&version).unwrap(),
            ReadObjectView::Version { .. }
        ));

        let _ = fs::remove_dir_all(&dir);
    }
}
