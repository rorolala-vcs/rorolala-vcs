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
#[lazyffi(export = locate_rola_inverse_index)]
pub fn locate_inverse_index(dir: &Path) -> Option<InverseIndex> {
    InverseIndex::locate(dir)
}

#[lazyffi(export = rola_inverse_index_)]
impl InverseIndex {
    /// Builds the records afresh from every object the index holds, and writes them.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexError`] if the index cannot be read or the records cannot be written.
    ///
    /// # FFI
    /// Blocks until the rebuild is done.
    #[lazyffi(export = rebuild_rola_inverse_index)]
    pub fn rebuild_blocking(&self) -> Result<InverseIndexReport, InverseIndexError> {
        run(self.rebuild()).map_err(|error| InverseIndexError::Io(error.into()))?
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
    #[lazyffi(export = get_store_dependents)]
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
    #[lazyffi(export = get_variant_dependents)]
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
    #[lazyffi(export = get_version_dependents)]
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
    #[lazyffi(export = get_child_versions)]
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
    #[lazyffi(export = get_creator_dependents)]
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
    #[lazyffi(export = get_message_dependents)]
    pub fn message_dependents_blocking(
        &self,
        hash: &str,
    ) -> Result<HashList, InverseIndexReadingError> {
        let key = key_of(hash)?;

        Ok(run(self.message_dependents(key))
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
    #[lazyffi(export = get_version_number)]
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
    /// How many objects the index holds.
    #[must_use]
    #[lazyffi(export = inverse_index_report_objects)]
    pub const fn objects(&self) -> u64 {
        self.objects
    }

    /// How many of them are variants.
    #[must_use]
    #[lazyffi(export = inverse_index_report_variants)]
    pub const fn variants(&self) -> u64 {
        self.variants
    }

    /// How many are versions.
    #[must_use]
    #[lazyffi(export = inverse_index_report_versions)]
    pub const fn versions(&self) -> u64 {
        self.versions
    }

    /// How many are creators.
    #[must_use]
    #[lazyffi(export = inverse_index_report_creators)]
    pub const fn creators(&self) -> u64 {
        self.creators
    }

    /// How many are messages.
    #[must_use]
    #[lazyffi(export = inverse_index_report_messages)]
    pub const fn messages(&self) -> u64 {
        self.messages
    }

    /// How many records the inverse index has.
    #[must_use]
    #[lazyffi(export = inverse_index_report_entries)]
    pub const fn entries(&self) -> u64 {
        self.entries
    }

    /// How many dependents are noted across the records.
    #[must_use]
    #[lazyffi(export = inverse_index_report_dependents)]
    pub const fn dependents(&self) -> u64 {
        self.dependents
    }

    /// The digest of what was written, as hex.
    #[must_use]
    #[lazyffi(export = inverse_index_report_fingerprint)]
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
