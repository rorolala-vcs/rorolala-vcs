//! The C ABI of an inverse index.
//!
//! Every question the Rust side asks asynchronously is asked here synchronously: an export blocks
//! on a runtime of its own, the way [`VCSIndex`](rorolala_vcs::VCSIndex)'s exports do, so that a C
//! caller reaches the same answers without knowing there is an async runtime underneath.

use std::future::Future;
use std::io;
use std::path::Path;
use std::str::FromStr as _;

use rorolala_utils_lazyffi::lazyffi;
use rorolala_vcs::Hash;

use crate::{
    HashList, InverseIndex, InverseIndexError, InverseIndexReadingError, InverseIndexReport,
    VersionNumber,
};

/// The inverse index of the Vault or Workspace `dir` is inside, if it is inside one.
#[must_use]
#[lazyffi(export = rola_inverse_index_locate)]
pub fn locate_inverse_index(dir: &Path) -> Option<InverseIndex> {
    InverseIndex::locate(dir)
}

#[lazyffi(export = rola_inverse_index_)]
impl InverseIndex {
    /// Brings the records up to date with the objects the index holds, reading only what is new.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexError`] if the index cannot be read or the records cannot be written.
    ///
    /// # FFI
    /// Blocks until the rebuild is done.
    #[lazyffi(export = rola_inverse_index_rebuild)]
    pub fn rebuild_blocking(&self) -> Result<InverseIndexReport, InverseIndexError> {
        run(self.rebuild()).map_err(|error| InverseIndexError::Io(error.into()))?
    }

    /// Builds the records afresh from every object the index holds, and writes them.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexError`] if the index cannot be read or the records cannot be written.
    ///
    /// # FFI
    /// Blocks until the rebuild is done.
    #[lazyffi(export = rola_inverse_index_full_rebuild)]
    pub fn rebuild_full_blocking(&self) -> Result<InverseIndexReport, InverseIndexError> {
        run(self.rebuild_full()).map_err(|error| InverseIndexError::Io(error.into()))?
    }

    /// The variants made from the content stored under `hash`.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexReadingError`] if `hash` does not read as a key or the index cannot be
    /// read.
    ///
    /// # FFI
    /// Blocks until the read is done. `hash` is the hex a key is written as.
    #[lazyffi(export = rola_inverse_index_store_dependents)]
    pub fn store_dependents_blocking(
        &self,
        hash: &str,
    ) -> Result<HashList, InverseIndexReadingError> {
        let key = key_of(hash)?;

        Ok(run(self.store_dependents(key))
            .map_err(|error| InverseIndexReadingError::Io(error.into()))??
            .into())
    }

    /// The versions that fix the variant named `hash`.
    ///
    /// # Errors
    ///
    /// As [`store_dependents_blocking`](Self::store_dependents_blocking).
    ///
    /// # FFI
    /// Blocks until the read is done. `hash` is the hex a key is written as.
    #[lazyffi(export = rola_inverse_index_variant_dependents)]
    pub fn variant_dependents_blocking(
        &self,
        hash: &str,
    ) -> Result<HashList, InverseIndexReadingError> {
        let key = key_of(hash)?;

        Ok(run(self.variant_dependents(key))
            .map_err(|error| InverseIndexReadingError::Io(error.into()))??
            .into())
    }

    /// The variants based on the version named `hash`.
    ///
    /// # Errors
    ///
    /// As [`store_dependents_blocking`](Self::store_dependents_blocking).
    ///
    /// # FFI
    /// Blocks until the read is done. `hash` is the hex a key is written as.
    #[lazyffi(export = rola_inverse_index_version_dependents)]
    pub fn version_dependents_blocking(
        &self,
        hash: &str,
    ) -> Result<HashList, InverseIndexReadingError> {
        let key = key_of(hash)?;

        Ok(run(self.version_dependents(key))
            .map_err(|error| InverseIndexReadingError::Io(error.into()))??
            .into())
    }

    /// The versions based on the version named `hash`.
    ///
    /// # Errors
    ///
    /// As [`store_dependents_blocking`](Self::store_dependents_blocking).
    ///
    /// # FFI
    /// Blocks until the read is done. `hash` is the hex a key is written as.
    #[lazyffi(export = rola_inverse_index_child_versions)]
    pub fn child_versions_blocking(
        &self,
        hash: &str,
    ) -> Result<HashList, InverseIndexReadingError> {
        let key = key_of(hash)?;

        Ok(run(self.child_versions(key))
            .map_err(|error| InverseIndexReadingError::Io(error.into()))??
            .into())
    }

    /// The variants made by the creator named `hash`.
    ///
    /// # Errors
    ///
    /// As [`store_dependents_blocking`](Self::store_dependents_blocking).
    ///
    /// # FFI
    /// Blocks until the read is done. `hash` is the hex a key is written as.
    #[lazyffi(export = rola_inverse_index_creator_dependents)]
    pub fn creator_dependents_blocking(
        &self,
        hash: &str,
    ) -> Result<HashList, InverseIndexReadingError> {
        let key = key_of(hash)?;

        Ok(run(self.creator_dependents(key))
            .map_err(|error| InverseIndexReadingError::Io(error.into()))??
            .into())
    }

    /// The variants that say what the message named `hash` says.
    ///
    /// # Errors
    ///
    /// As [`store_dependents_blocking`](Self::store_dependents_blocking).
    ///
    /// # FFI
    /// Blocks until the read is done. `hash` is the hex a key is written as.
    #[lazyffi(export = rola_inverse_index_message_dependents)]
    pub fn message_dependents_blocking(
        &self,
        hash: &str,
    ) -> Result<HashList, InverseIndexReadingError> {
        let key = key_of(hash)?;

        Ok(run(self.message_dependents(key))
            .map_err(|error| InverseIndexReadingError::Io(error.into()))??
            .into())
    }

    /// The variants that merge in the variant named `hash`.
    ///
    /// # Errors
    ///
    /// As [`store_dependents_blocking`](Self::store_dependents_blocking).
    ///
    /// # FFI
    /// Blocks until the read is done. `hash` is the hex a key is written as.
    #[lazyffi(export = rola_inverse_index_join_dependents)]
    pub fn join_dependents_blocking(
        &self,
        hash: &str,
    ) -> Result<HashList, InverseIndexReadingError> {
        let key = key_of(hash)?;

        Ok(run(self.join_dependents(key))
            .map_err(|error| InverseIndexReadingError::Io(error.into()))??
            .into())
    }

    /// The number of the version named `hash`.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexReadingError`] if `hash` does not read as a key, nothing is stored
    /// under it, it is not a version, or the index cannot be read.
    ///
    /// # FFI
    /// Blocks until the read is done. `hash` is the hex a key is written as.
    #[lazyffi(export = rola_inverse_index_version_number)]
    pub fn version_num_blocking(
        &self,
        hash: &str,
    ) -> Result<VersionNumber, InverseIndexReadingError> {
        let key = key_of(hash)?;
        let number = run(self.version_num(key))
            .map_err(|error| InverseIndexReadingError::Io(error.into()))??;

        Ok(VersionNumber::from(number))
    }
}

#[lazyffi(export = rola_inverse_index_report_)]
impl InverseIndexReport {
    /// What the rebuild had to do: `fresh`, `incremental` or `full`.
    #[must_use]
    #[lazyffi(export = rola_inverse_index_report_mode)]
    pub fn mode(&self) -> String {
        self.mode.name().to_owned()
    }

    /// How many objects the index holds, as the build left it.
    #[must_use]
    #[lazyffi(export = rola_inverse_index_report_objects)]
    pub const fn objects(&self) -> u64 {
        self.objects
    }

    /// How many objects this rebuild read.
    #[must_use]
    #[lazyffi(export = rola_inverse_index_report_objects_read)]
    pub const fn objects_read(&self) -> u64 {
        self.objects_read
    }

    /// How many of them are variants.
    #[must_use]
    #[lazyffi(export = rola_inverse_index_report_variants)]
    pub const fn variants(&self) -> u64 {
        self.variants
    }

    /// How many are versions.
    #[must_use]
    #[lazyffi(export = rola_inverse_index_report_versions)]
    pub const fn versions(&self) -> u64 {
        self.versions
    }

    /// How many are creators.
    #[must_use]
    #[lazyffi(export = rola_inverse_index_report_creators)]
    pub const fn creators(&self) -> u64 {
        self.creators
    }

    /// How many are messages.
    #[must_use]
    #[lazyffi(export = rola_inverse_index_report_messages)]
    pub const fn messages(&self) -> u64 {
        self.messages
    }

    /// How many versions this rebuild numbered itself, the root's own number included.
    #[must_use]
    #[lazyffi(export = rola_inverse_index_report_versions_numbered)]
    pub const fn versions_numbered(&self) -> u64 {
        self.versions_numbered
    }

    /// How many records the inverse index has.
    #[must_use]
    #[lazyffi(export = rola_inverse_index_report_entries)]
    pub const fn entries(&self) -> u64 {
        self.entries
    }

    /// How many dependents are noted across the records.
    #[must_use]
    #[lazyffi(export = rola_inverse_index_report_dependents)]
    pub const fn dependents(&self) -> u64 {
        self.dependents
    }

    /// The digest of what was written, as hex.
    #[must_use]
    #[lazyffi(export = rola_inverse_index_report_fingerprint)]
    pub fn fingerprint(&self) -> String {
        self.fingerprint.clone()
    }
}

/// The key `hash` names, or a read that was handed something that is not one.
fn key_of(hash: &str) -> Result<Hash, InverseIndexReadingError> {
    Hash::from_str(hash).map_err(|_| InverseIndexReadingError::Malformed)
}

/// Waits for `future` on a runtime of this call's own, so a synchronous export can drive an
/// asynchronous read or build.
fn run<T>(future: impl Future<Output = T>) -> Result<T, io::Error> {
    let runtime = tokio::runtime::Runtime::new()?;

    Ok(runtime.block_on(future))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_vcs::VCSIndex;

    use crate::{InverseIndex, InverseIndexReadingError, InverseIndexReport, RebuildMode};

    use super::{key_of, locate_inverse_index, run};

    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rola-inverse-index-ffi-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        dir
    }

    fn index(label: &str) -> (PathBuf, InverseIndex) {
        let dir = scratch(label);
        let index = InverseIndex::at(VCSIndex::create(dir.clone()));

        (dir, index)
    }

    #[test]
    fn an_invalid_hash_is_refused_before_the_index_is_read() {
        let (dir, index) = index("hash");

        assert!(matches!(
            index.store_dependents_blocking("not a hash"),
            Err(InverseIndexReadingError::Malformed)
        ));
        assert!(matches!(
            index.variant_dependents_blocking("not a hash"),
            Err(InverseIndexReadingError::Malformed)
        ));
        assert!(matches!(
            index.version_dependents_blocking("not a hash"),
            Err(InverseIndexReadingError::Malformed)
        ));
        assert!(matches!(
            index.child_versions_blocking("not a hash"),
            Err(InverseIndexReadingError::Malformed)
        ));
        assert!(matches!(
            index.creator_dependents_blocking("not a hash"),
            Err(InverseIndexReadingError::Malformed)
        ));
        assert!(matches!(
            index.message_dependents_blocking("not a hash"),
            Err(InverseIndexReadingError::Malformed)
        ));
        assert!(matches!(
            index.join_dependents_blocking("not a hash"),
            Err(InverseIndexReadingError::Malformed)
        ));
        assert!(matches!(
            index.version_num_blocking("not a hash"),
            Err(InverseIndexReadingError::Malformed)
        ));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_key_is_read_from_the_hex_a_caller_writes() {
        let hex = "a".repeat(64);
        let key = key_of(&hex).unwrap();

        assert_eq!(key.hex(), hex);
        assert!(matches!(
            key_of("not a hash"),
            Err(InverseIndexReadingError::Malformed)
        ));
        assert_eq!(run(async { 7 }).unwrap(), 7);
    }

    #[test]
    fn a_report_answers_each_count_it_was_given() {
        let report = InverseIndexReport {
            mode: RebuildMode::Full,
            objects: 1,
            objects_read: 2,
            variants: 3,
            versions: 4,
            creators: 5,
            messages: 6,
            versions_numbered: 7,
            entries: 8,
            dependents: 9,
            fingerprint: "hex".to_owned(),
        };

        assert_eq!(report.mode(), "full");
        assert_eq!(report.objects(), 1);
        assert_eq!(report.objects_read(), 2);
        assert_eq!(report.variants(), 3);
        assert_eq!(report.versions(), 4);
        assert_eq!(report.creators(), 5);
        assert_eq!(report.messages(), 6);
        assert_eq!(report.versions_numbered(), 7);
        assert_eq!(report.entries(), 8);
        assert_eq!(report.dependents(), 9);
        assert_eq!(report.fingerprint(), "hex");
    }

    #[test]
    fn a_directory_with_no_index_holds_no_inverse_index() {
        let dir = scratch("locate");

        assert!(locate_inverse_index(&dir).is_none());

        let _ = std::fs::remove_dir_all(dir);
    }
}
