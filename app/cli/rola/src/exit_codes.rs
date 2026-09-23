//! The exit codes the program ends with.
//!
//! Each constant states a number and carries the `i18n/exit_codes.yml` key that says what
//! it means, written above it. `build.rs` reads the keys and generates `explain.rs` beside
//! this file, which is what turns a number back into words — so the words live in the
//! locale files and the keys live here, and neither is written twice.

mod explain;

pub use explain::*;

// General

/// `exit_codes.already_exist`
pub const EC_ALREADY_EXIST: i32 = 11;

/// `exit_codes.not_exist`
pub const EC_NOT_EXIST: i32 = 12;

/// `exit_codes.unknown_command`
pub const EC_UNKNOWN_COMMAND: i32 = 13;

/// `exit_codes.help`
pub const EC_HELP: i32 = 2;

// Creation

/// `exit_codes.creation_workspace`
pub const EC_ERR_CREATION_WORKSPACE: i32 = 21;

/// `exit_codes.creation_vault`
pub const EC_ERR_CREATION_VAULT: i32 = 22;

/// `exit_codes.creation_argument`
pub const EC_ERR_CREATION_ARGUMENT: i32 = 23;

// Tools

/// `exit_codes.tool_keygen_no_openssl`
pub const EC_ERR_TOOL_KEYGEN_NO_OPENSSL: i32 = 41;

/// `exit_codes.tool_keygen_failed`
pub const EC_ERR_TOOL_KEYGEN_FAILED: i32 = 42;

/// `exit_codes.tool_keygen_path_not_exist`
pub const EC_ERR_TOOL_KEYGEN_PATH_NOT_EXIST: i32 = 43;

/// `exit_codes.tool_keygen_no_key_dir`
pub const EC_ERR_TOOL_KEYGEN_NO_KEY_DIR: i32 = 44;

/// `exit_codes.tool_keygen_install_failed`
pub const EC_ERR_TOOL_KEYGEN_INSTALL_FAILED: i32 = 45;

/// `exit_codes.tool_write_file_argument`
pub const EC_ERR_TOOL_WRITE_FILE_ARGUMENT: i32 = 46;

/// `exit_codes.tool_write_file_no_storage`
pub const EC_ERR_TOOL_WRITE_FILE_NO_STORAGE: i32 = 47;

/// `exit_codes.tool_write_file_not_a_file`
pub const EC_ERR_TOOL_WRITE_FILE_NOT_A_FILE: i32 = 48;

/// `exit_codes.tool_write_file_failed`
pub const EC_ERR_TOOL_WRITE_FILE_FAILED: i32 = 49;

/// `exit_codes.tool_extract_file_argument`
pub const EC_ERR_TOOL_EXTRACT_FILE_ARGUMENT: i32 = 54;

/// `exit_codes.tool_extract_file_no_storage`
pub const EC_ERR_TOOL_EXTRACT_FILE_NO_STORAGE: i32 = 55;

/// `exit_codes.tool_extract_file_bad_hash`
pub const EC_ERR_TOOL_EXTRACT_FILE_BAD_HASH: i32 = 56;

/// `exit_codes.tool_extract_file_exists`
pub const EC_ERR_TOOL_EXTRACT_FILE_EXISTS: i32 = 57;

/// `exit_codes.tool_extract_file_failed`
pub const EC_ERR_TOOL_EXTRACT_FILE_FAILED: i32 = 58;

// Packing

/// `exit_codes.pack_no_storage`
pub const EC_ERR_PACK_NO_STORAGE: i32 = 59;

/// `exit_codes.pack_failed`
pub const EC_ERR_PACK_FAILED: i32 = 60;

// Listing

/// `exit_codes.tool_ls_storaged_no_storage`
pub const EC_ERR_TOOL_LS_STORAGED_NO_STORAGE: i32 = 64;

/// `exit_codes.tool_ls_storaged_failed`
pub const EC_ERR_TOOL_LS_STORAGED_FAILED: i32 = 65;

/// `exit_codes.tool_ls_manifests_no_storage`
pub const EC_ERR_TOOL_LS_MANIFESTS_NO_STORAGE: i32 = 66;

/// `exit_codes.tool_ls_manifests_failed`
pub const EC_ERR_TOOL_LS_MANIFESTS_FAILED: i32 = 67;

// Configuration

/// `exit_codes.config_unreadable`
pub const EC_ERR_CONFIG_UNREADABLE: i32 = 51;

// Vaults

/// `exit_codes.vault_argument`
pub const EC_ERR_VAULT_ARGUMENT: i32 = 52;

/// `exit_codes.vault_not_bound`
pub const EC_ERR_VAULT_NOT_BOUND: i32 = 53;

// Accounts

/// `exit_codes.account_not_found`
pub const EC_ERR_ACCOUNT_NOT_FOUND: i32 = 61;

/// `exit_codes.account_no_dir`
pub const EC_ERR_ACCOUNT_NO_DIR: i32 = 62;

/// `exit_codes.account_not_bound`
pub const EC_ERR_ACCOUNT_NOT_BOUND: i32 = 63;

// Placement

/// `exit_codes.should_in_workspace`
pub const EC_ERR_SHOULD_IN_WORKSPACE: i32 = 71;

/// `exit_codes.should_in_vault`
pub const EC_ERR_SHOULD_IN_VAULT: i32 = 72;

/// `exit_codes.no_remote_vault`
pub const EC_ERR_NO_REMOTE_VAULT: i32 = 73;

// Actions

/// `exit_codes.action_no_channel`
pub const EC_ERR_ACTION_NO_CHANNEL: i32 = 81;

/// `exit_codes.action_missing_value`
pub const EC_ERR_ACTION_MISSING_VALUE: i32 = 82;

/// `exit_codes.action_value_too_large`
pub const EC_ERR_ACTION_VALUE_TOO_LARGE: i32 = 83;

/// `exit_codes.action_io`
pub const EC_ERR_ACTION_IO: i32 = 84;

/// `exit_codes.action_codec`
pub const EC_ERR_ACTION_CODEC: i32 = 85;

/// `exit_codes.action_unknown`
pub const EC_ERR_ACTION_UNKNOWN: i32 = 86;

/// `exit_codes.action_json`
pub const EC_ERR_ACTION_JSON: i32 = 87;

/// `exit_codes.action_addr`
pub const EC_ERR_ACTION_ADDR: i32 = 88;

/// `exit_codes.action_auth`
pub const EC_ERR_ACTION_AUTH: i32 = 89;

/// `exit_codes.action_missing_object`
pub const EC_ERR_ACTION_MISSING_OBJECT: i32 = 90;

/// `exit_codes.action_store`
pub const EC_ERR_ACTION_STORE: i32 = 93;

// Explain

/// `exit_codes.explain_unknown`
pub const EC_ERR_EXPLAIN_UNKNOWN: i32 = 91;

/// `exit_codes.explain_no_lastec`
pub const EC_ERR_EXPLAIN_NO_LASTEC: i32 = 92;
