//! What the store is told by the file that makes it a store.
//!
//! The file is the store's own — `rolast.toml`, the same one a search finds the store by — and what
//! it says is read here rather than in the places that act on it, so a caller asks a store what it
//! was told instead of knowing where that is written down.

use super::RorolalaStorage;
use crate::StorageConfig;

impl RorolalaStorage {
    /// The configuration this store works from.
    ///
    /// A configuration that cannot be read — no file, or one that does not read — is the empty one,
    /// so what a store does not say is defaulted rather than refused.
    #[must_use]
    pub async fn config(&self) -> StorageConfig {
        tokio::fs::read_to_string(self.config_path())
            .await
            .map_or_else(
                |_| StorageConfig::default(),
                |text| StorageConfig::parse(&text),
            )
    }

    /// The largest a pack may grow before another is started.
    ///
    /// A store that says nothing about it takes
    /// [`DEFAULT_MAX_PACK_SIZE`](crate::DEFAULT_MAX_PACK_SIZE).
    #[must_use]
    pub async fn max_pack_size(&self) -> u64 {
        self.config().await.max_pack_size()
    }
}
