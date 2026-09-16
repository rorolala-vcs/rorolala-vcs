use rorolala_utils_configure::Config;
use rorolala_utils_lazyffi::lazyffi;
use std::path::Path;

use crate::{CONFIG_PATH, Vault, error::CreationError};

#[lazyffi]
impl Vault {
    /// Creates a Vault in the specified directory
    ///
    /// # Errors
    ///
    /// Returns [`CreationError`] if the configuration
    /// file cannot be created at the given directory.
    #[lazyffi(export = create_vault)]
    pub fn create(dir: &Path) -> Result<(), CreationError> {
        let config = Config::<crate::config::Config>::new(dir.join(CONFIG_PATH)).map_err(
            |error| match error {
                rorolala_utils_configure::Error::Locked { .. } => CreationError::ConfigLocked,
                rorolala_utils_configure::Error::Render { .. } => CreationError::ConfigRenderFailed,
                rorolala_utils_configure::Error::Stage { .. } => CreationError::ConfigStageFailed,
                rorolala_utils_configure::Error::Publish { .. } => {
                    CreationError::ConfigPublishFailed
                }
                _ => CreationError::UnknownError,
            },
        )?;

        config.write().map_err(|error| match error {
            rorolala_utils_configure::Error::Render { .. } => {
                CreationError::ConfigWriteRenderFailed
            }
            rorolala_utils_configure::Error::Stage { .. } => CreationError::ConfigWriteStageFailed,
            _ => CreationError::UnknownError,
        })?;

        // Drop config to write it to the filesystem
        drop(config);
        Ok(())
    }
}
