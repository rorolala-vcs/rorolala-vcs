use rorolala_utils_lazyffi::lazyffi;

/// Represents an error that occurs during the creation of a Workspace
#[lazyffi(export = WorkspaceCreationError)]
#[derive(Debug)]
pub enum CreationError {
    /// Failure when the directory the Workspace keeps its data in cannot be created
    DataDirCreateFailed,
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
    /// As for a Vault's: every way of creating a Workspace fails in its own right, and a
    /// reader that is told which one it was has something to do about it.
    fn name(&self) -> &'static str {
        match self {
            Self::DataDirCreateFailed => "workspace_creation_data_dir_create_failed",
            Self::ConfigLocked => "workspace_creation_config_locked",
            Self::ConfigRenderFailed => "workspace_creation_config_render_failed",
            Self::ConfigStageFailed => "workspace_creation_config_stage_failed",
            Self::ConfigPublishFailed => "workspace_creation_config_publish_failed",
            Self::ConfigWriteRenderFailed => "workspace_creation_config_write_render_failed",
            Self::ConfigWriteStageFailed => "workspace_creation_config_write_stage_failed",
            Self::UnknownError => "workspace_creation_unknown_error",
        }
    }

    /// What went wrong, in the library's own words.
    ///
    /// As for a Vault's: what a person is shown is said where the command is, in the run's
    /// language, and this is only what the failure can say about itself.
    fn reason(&self) -> String {
        let reason = match self {
            Self::DataDirCreateFailed => {
                "the directory the Workspace keeps its data in could not be created"
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
