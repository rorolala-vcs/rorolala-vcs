use rorolala_utils_lazyffi::lazyffi;

/// Represents an error that occurs during the creation of a Vault
#[lazyffi(export = VaultCreationError)]
#[derive(Debug)]
pub enum CreationError {
    /// Failure when the directory the Vault keeps its configuration in cannot be created
    DirCreateFailed,
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

impl rorolala_errors::Failure for CreationError {
    /// The name of the way this failed, as a program reads it.
    ///
    /// Every way of creating a Vault fails in its own right — a directory that will not be
    /// made is not a configuration that will not be written — and a reader that is told
    /// which one it was has something to do about it. What they have in common is only the
    /// exit code, which is why they are named apart here.
    fn name(&self) -> &'static str {
        match self {
            Self::DirCreateFailed => "vault_creation_dir_create_failed",
            Self::ConfigLocked => "vault_creation_config_locked",
            Self::ConfigRenderFailed => "vault_creation_config_render_failed",
            Self::ConfigStageFailed => "vault_creation_config_stage_failed",
            Self::ConfigPublishFailed => "vault_creation_config_publish_failed",
            Self::ConfigWriteRenderFailed => "vault_creation_config_write_render_failed",
            Self::ConfigWriteStageFailed => "vault_creation_config_write_stage_failed",
            Self::UnknownError => "vault_creation_unknown_error",
        }
    }

    /// What went wrong, in the library's own words.
    ///
    /// A library has one voice and speaks in it; a command that shows this to a person says
    /// it in the run's language itself. What is handed over here is what the failure can say
    /// about itself, which is the same thing [`Debug`] names and no more.
    fn reason(&self) -> String {
        let reason = match self {
            Self::DirCreateFailed => {
                "the directory the Vault keeps its configuration in could not be created"
            }
            Self::ConfigLocked => {
                "the staging file could not be read to check whether it already exists"
            }
            Self::ConfigRenderFailed => {
                "the default value could not be parsed into the configuration format"
            }
            Self::ConfigStageFailed => {
                "the staging file the configuration is made in could not be written"
            }
            Self::ConfigPublishFailed => {
                "the staging file could not be published to the configuration file"
            }
            Self::ConfigWriteRenderFailed => "the staging file could not be rendered",
            Self::ConfigWriteStageFailed => {
                "the staging file the configuration is saved in could not be written"
            }
            Self::UnknownError => "an unknown or unclassified failure",
        };

        reason.to_owned()
    }
}

rorolala_errors::failure!(CreationError);
