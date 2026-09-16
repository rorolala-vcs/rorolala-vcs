use rorolala_utils_lazyffi::lazyffi;

/// Represents an error that occurs during the creation of a Vault
#[lazyffi(export = VaultCreationError)]
pub enum CreationError {
    /// Failure when reading the staging file to check if it already exists
    ConfigLocked,
    /// Failure when rendering the default value into the configuration format
    ConfigRenderFailed,
    /// Failure when writing the staging file
    ConfigStageFailed,
    /// Failure when publishing (copying) the staging file to the configuration file
    ConfigPublishFailed,
    /// Failure when the staging file cannot be rendered
    ConfigWriteRenderFailed,
    /// Failure when the staging file cannot be written
    ConfigWriteStageFailed,

    /// Failure with an unknown or unclassified cause
    UnknownError,
}
