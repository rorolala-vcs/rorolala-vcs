use mingling::{
    macros::{buffer, import_type, r_eprintln, renderer},
    res::ResExitCode,
};
use rorolala_utils_cli_theme::{err_line, help_line};
use rust_i18n::t;

use crate::exit_codes::{EC_ERR_CREATION_VAULT, EC_ERR_CREATION_WORKSPACE};

import_type!(ErrorVaultCreation = rorolala_vault::CreationError);

#[renderer(buffer)]
pub fn render_error_vault_creation(err: ErrorVaultCreation, ec: &mut ResExitCode) {
    match err {
        rorolala_vault::CreationError::ConfigLocked => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.vault_creation.err_config_locked")).trim()
            );
            r_eprintln!(
                "{}",
                help_line!(t!("error.vault_creation.err_config_locked_help")).trim()
            );
        }
        rorolala_vault::CreationError::ConfigRenderFailed => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.vault_creation.err_config_render_failed")).trim()
            );
            r_eprintln!(
                "{}",
                help_line!(t!("error.vault_creation.err_config_render_failed_help")).trim()
            );
        }
        rorolala_vault::CreationError::ConfigStageFailed => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.vault_creation.err_config_stage_failed")).trim()
            );
            r_eprintln!(
                "{}",
                help_line!(t!("error.vault_creation.err_config_stage_failed_help")).trim()
            );
        }
        rorolala_vault::CreationError::ConfigPublishFailed => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.vault_creation.err_config_publish_failed")).trim()
            );
            r_eprintln!(
                "{}",
                help_line!(t!("error.vault_creation.err_config_publish_failed_help")).trim()
            );
        }
        rorolala_vault::CreationError::ConfigWriteRenderFailed => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.vault_creation.err_config_write_render_failed")).trim()
            );
            r_eprintln!(
                "{}",
                help_line!(t!(
                    "error.vault_creation.err_config_write_render_failed_help"
                ))
                .trim()
            );
        }
        rorolala_vault::CreationError::ConfigWriteStageFailed => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.vault_creation.err_config_write_stage_failed")).trim()
            );
            r_eprintln!(
                "{}",
                help_line!(t!(
                    "error.vault_creation.err_config_write_stage_failed_help"
                ))
                .trim()
            );
        }
        rorolala_vault::CreationError::UnknownError => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.vault_creation.err_unknown_error")).trim()
            );
            r_eprintln!(
                "{}",
                help_line!(t!("error.vault_creation.err_unknown_error_help")).trim()
            );
        }
    }
    ec.exit_code = EC_ERR_CREATION_VAULT;
}

import_type!(ErrorWorkspaceCreation = rorolala_workspace::CreationError);

#[renderer(buffer)]
pub fn render_error_workspace_creation(err: ErrorWorkspaceCreation, ec: &mut ResExitCode) {
    match err {
        rorolala_workspace::CreationError::DataDirCreateFailed => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.workspace_creation.err_data_dir_create_failed")).trim()
            );
            r_eprintln!(
                "{}",
                help_line!(t!(
                    "error.workspace_creation.err_data_dir_create_failed_help"
                ))
                .trim()
            );
        }
        rorolala_workspace::CreationError::ConfigLocked => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.workspace_creation.err_config_locked")).trim()
            );
            r_eprintln!(
                "{}",
                help_line!(t!("error.workspace_creation.err_config_locked_help")).trim()
            );
        }
        rorolala_workspace::CreationError::ConfigRenderFailed => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.workspace_creation.err_config_render_failed")).trim()
            );
            r_eprintln!(
                "{}",
                help_line!(t!("error.workspace_creation.err_config_render_failed_help")).trim()
            );
        }
        rorolala_workspace::CreationError::ConfigStageFailed => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.workspace_creation.err_config_stage_failed")).trim()
            );
            r_eprintln!(
                "{}",
                help_line!(t!("error.workspace_creation.err_config_stage_failed_help")).trim()
            );
        }
        rorolala_workspace::CreationError::ConfigPublishFailed => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.workspace_creation.err_config_publish_failed")).trim()
            );
            r_eprintln!(
                "{}",
                help_line!(t!(
                    "error.workspace_creation.err_config_publish_failed_help"
                ))
                .trim()
            );
        }
        rorolala_workspace::CreationError::ConfigWriteRenderFailed => {
            r_eprintln!(
                "{}",
                err_line!(t!(
                    "error.workspace_creation.err_config_write_render_failed"
                ))
                .trim()
            );
            r_eprintln!(
                "{}",
                help_line!(t!(
                    "error.workspace_creation.err_config_write_render_failed_help"
                ))
                .trim()
            );
        }
        rorolala_workspace::CreationError::ConfigWriteStageFailed => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.workspace_creation.err_config_write_stage_failed")).trim()
            );
            r_eprintln!(
                "{}",
                help_line!(t!(
                    "error.workspace_creation.err_config_write_stage_failed_help"
                ))
                .trim()
            );
        }
        rorolala_workspace::CreationError::UnknownError => {
            r_eprintln!(
                "{}",
                err_line!(t!("error.workspace_creation.err_unknown_error")).trim()
            );
            r_eprintln!(
                "{}",
                help_line!(t!("error.workspace_creation.err_unknown_error_help")).trim()
            );
        }
    }
    ec.exit_code = EC_ERR_CREATION_WORKSPACE;
}
