//! The exit codes the program ends with.
//!
//! Each constant states a number and carries the `i18n/exit_codes.yml` key that says what
//! it means, written above it. `build.rs` reads the keys and generates `explain.rs` beside
//! this file, which is what turns a number back into words — so the words live in the
//! locale files and the keys live here, and neither is written twice.
//!
//! The numbers are laid out a block to a subject, ten apart, in the order a run meets them:
//! what ended without doing anything wrong first, then the shapes of a run — where it is,
//! who it is, what it reaches — then the content it moves, then the version control
//! surfaces, then the machinery and tools. A block is held whole rather than packed against
//! the one before it, so a subject grows by taking the next number in its own block instead
//! of pushing every block after it along.

mod explain;

pub use explain::*;

// Informational — a run that ended without doing anything wrong

/// `exit_codes.help`
pub const EC_HELP: i32 = 2;

/// `exit_codes.cancelled`
pub const EC_CANCELLED: i32 = 3;

// General

/// `exit_codes.already_exist`
pub const EC_ALREADY_EXIST: i32 = 10;

/// `exit_codes.not_exist`
pub const EC_NOT_EXIST: i32 = 11;

/// `exit_codes.unknown_command`
pub const EC_UNKNOWN_COMMAND: i32 = 12;

// Creation

/// `exit_codes.creation_workspace`
pub const EC_ERR_CREATION_WORKSPACE: i32 = 20;

/// `exit_codes.creation_vault`
pub const EC_ERR_CREATION_VAULT: i32 = 21;

/// `exit_codes.creation_argument`
pub const EC_ERR_CREATION_ARGUMENT: i32 = 22;

// Placement — where a run has to be

/// `exit_codes.should_in_workspace`
pub const EC_ERR_SHOULD_IN_WORKSPACE: i32 = 30;

/// `exit_codes.should_in_vault`
pub const EC_ERR_SHOULD_IN_VAULT: i32 = 31;

/// `exit_codes.no_remote_vault`
pub const EC_ERR_NO_REMOTE_VAULT: i32 = 32;

// Configuration

/// `exit_codes.config_unreadable`
pub const EC_ERR_CONFIG_UNREADABLE: i32 = 40;

// Keys

/// `exit_codes.keygen_no_openssl`
pub const EC_ERR_KEYGEN_NO_OPENSSL: i32 = 50;

/// `exit_codes.keygen_failed`
pub const EC_ERR_KEYGEN_FAILED: i32 = 51;

/// `exit_codes.keygen_no_key_dir`
pub const EC_ERR_KEYGEN_NO_KEY_DIR: i32 = 52;

/// `exit_codes.keygen_install_failed`
pub const EC_ERR_KEYGEN_INSTALL_FAILED: i32 = 53;

/// `exit_codes.key_argument`
pub const EC_ERR_KEY_ARGUMENT: i32 = 54;

// Accounts

/// `exit_codes.account_not_found`
pub const EC_ERR_ACCOUNT_NOT_FOUND: i32 = 60;

/// `exit_codes.account_no_dir`
pub const EC_ERR_ACCOUNT_NO_DIR: i32 = 61;

/// `exit_codes.account_not_bound`
pub const EC_ERR_ACCOUNT_NOT_BOUND: i32 = 62;

// Vaults

/// `exit_codes.vault_argument`
pub const EC_ERR_VAULT_ARGUMENT: i32 = 70;

/// `exit_codes.vault_not_bound`
pub const EC_ERR_VAULT_NOT_BOUND: i32 = 71;

/// `exit_codes.vault_not_admin`
pub const EC_ERR_VAULT_NOT_ADMIN: i32 = 72;

// Storage — writing, reading back, and listing what a store holds

/// `exit_codes.storage_write_file_argument`
pub const EC_ERR_STORAGE_WRITE_FILE_ARGUMENT: i32 = 80;

/// `exit_codes.storage_write_file_no_storage`
pub const EC_ERR_STORAGE_WRITE_FILE_NO_STORAGE: i32 = 81;

/// `exit_codes.storage_write_file_not_a_file`
pub const EC_ERR_STORAGE_WRITE_FILE_NOT_A_FILE: i32 = 82;

/// `exit_codes.storage_write_file_failed`
pub const EC_ERR_STORAGE_WRITE_FILE_FAILED: i32 = 83;

/// `exit_codes.storage_extract_file_argument`
pub const EC_ERR_STORAGE_EXTRACT_FILE_ARGUMENT: i32 = 84;

/// `exit_codes.storage_extract_file_no_storage`
pub const EC_ERR_STORAGE_EXTRACT_FILE_NO_STORAGE: i32 = 85;

/// `exit_codes.storage_extract_file_bad_hash`
pub const EC_ERR_STORAGE_EXTRACT_FILE_BAD_HASH: i32 = 86;

/// `exit_codes.storage_extract_file_exists`
pub const EC_ERR_STORAGE_EXTRACT_FILE_EXISTS: i32 = 87;

/// `exit_codes.storage_extract_file_failed`
pub const EC_ERR_STORAGE_EXTRACT_FILE_FAILED: i32 = 88;

/// `exit_codes.storage_ls_storaged_no_storage`
pub const EC_ERR_STORAGE_LS_STORAGED_NO_STORAGE: i32 = 89;

/// `exit_codes.storage_ls_storaged_failed`
pub const EC_ERR_STORAGE_LS_STORAGED_FAILED: i32 = 90;

/// `exit_codes.storage_ls_manifests_no_storage`
pub const EC_ERR_STORAGE_LS_MANIFESTS_NO_STORAGE: i32 = 91;

/// `exit_codes.storage_ls_manifests_failed`
pub const EC_ERR_STORAGE_LS_MANIFESTS_FAILED: i32 = 92;

// Packing

/// `exit_codes.pack_no_storage`
pub const EC_ERR_PACK_NO_STORAGE: i32 = 100;

/// `exit_codes.pack_failed`
pub const EC_ERR_PACK_FAILED: i32 = 101;

/// `exit_codes.pack_locked`
pub const EC_ERR_PACK_LOCKED: i32 = 102;

/// `exit_codes.pack_nothing_to_pack`
pub const EC_ERR_PACK_NOTHING_TO_PACK: i32 = 103;

// File operations

/// `exit_codes.fs_ops_argument`
pub const EC_ERR_FS_OPS_ARGUMENT: i32 = 110;

/// `exit_codes.fs_ops_failed`
pub const EC_ERR_FS_OPS_FAILED: i32 = 111;

// Version control index

/// `exit_codes.vcs_index_no_index`
pub const EC_ERR_VCS_INDEX_NO_INDEX: i32 = 120;

/// `exit_codes.vcs_index_read`
pub const EC_ERR_VCS_INDEX_READ: i32 = 121;

/// `exit_codes.vcs_index_write`
pub const EC_ERR_VCS_INDEX_WRITE: i32 = 122;

/// `exit_codes.vcs_index_argument`
pub const EC_ERR_VCS_INDEX_ARGUMENT: i32 = 123;

/// `exit_codes.vcs_index_not_found`
pub const EC_ERR_VCS_INDEX_NOT_FOUND: i32 = 124;

// Version control inverse index

/// `exit_codes.inv_idx_no_index`
pub const EC_ERR_INV_IDX_NO_INDEX: i32 = 130;

/// `exit_codes.inv_idx_read`
pub const EC_ERR_INV_IDX_READ: i32 = 131;

/// `exit_codes.inv_idx_argument`
pub const EC_ERR_INV_IDX_ARGUMENT: i32 = 132;

/// `exit_codes.inv_idx_not_found`
pub const EC_ERR_INV_IDX_NOT_FOUND: i32 = 133;

// Actions — what the exchange between two ends fails with

/// `exit_codes.action_no_channel`
pub const EC_ERR_ACTION_NO_CHANNEL: i32 = 140;

/// `exit_codes.action_missing_value`
pub const EC_ERR_ACTION_MISSING_VALUE: i32 = 141;

/// `exit_codes.action_value_too_large`
pub const EC_ERR_ACTION_VALUE_TOO_LARGE: i32 = 142;

/// `exit_codes.action_io`
pub const EC_ERR_ACTION_IO: i32 = 143;

/// `exit_codes.action_codec`
pub const EC_ERR_ACTION_CODEC: i32 = 144;

/// `exit_codes.action_unknown`
pub const EC_ERR_ACTION_UNKNOWN: i32 = 145;

/// `exit_codes.action_json`
pub const EC_ERR_ACTION_JSON: i32 = 146;

/// `exit_codes.action_addr`
pub const EC_ERR_ACTION_ADDR: i32 = 147;

/// `exit_codes.action_auth`
pub const EC_ERR_ACTION_AUTH: i32 = 148;

/// `exit_codes.action_missing_object`
pub const EC_ERR_ACTION_MISSING_OBJECT: i32 = 149;

/// `exit_codes.action_store`
pub const EC_ERR_ACTION_STORE: i32 = 150;

// Explain

/// `exit_codes.explain_unknown`
pub const EC_ERR_EXPLAIN_UNKNOWN: i32 = 160;

/// `exit_codes.explain_no_lastec`
pub const EC_ERR_EXPLAIN_NO_LASTEC: i32 = 161;

// Desktop

/// `exit_codes.desktop_not_found`
pub const EC_ERR_DESKTOP_NOT_FOUND: i32 = 170;

/// `exit_codes.desktop_launch_failed`
pub const EC_ERR_DESKTOP_LAUNCH_FAILED: i32 = 171;

/// `exit_codes.desktop_ended`
pub const EC_ERR_DESKTOP_ENDED: i32 = 172;

// Output — how a result is drawn when it is written

/// `exit_codes.format`
pub const EC_ERR_FORMAT: i32 = 180;

// Layouts — the places a Workspace or a Vault works in

/// `exit_codes.layout`
pub const EC_ERR_LAYOUT: i32 = 190;

/// `exit_codes.layout_argument`
pub const EC_ERR_LAYOUT_ARGUMENT: i32 = 191;

/// `exit_codes.layout_not_cached`
pub const EC_ERR_LAYOUT_NOT_CACHED: i32 = 192;

/// `exit_codes.layout_ownership`
pub const EC_ERR_LAYOUT_OWNERSHIP: i32 = 193;

// Tracking — writing content and recording it in a Layout

/// `exit_codes.track`
pub const EC_ERR_TRACK: i32 = 200;

/// `exit_codes.track_argument`
pub const EC_ERR_TRACK_ARGUMENT: i32 = 201;

// Editing — a message a run cannot be told in one argument

/// `exit_codes.editor`
pub const EC_ERR_EDITOR: i32 = 210;

/// `exit_codes.abort`
pub const EC_ABORT: i32 = 211;

// Aligning — bringing the tree and the Layout back into agreement

/// `exit_codes.align`
pub const EC_ERR_ALIGN: i32 = 220;

/// `exit_codes.align_argument`
pub const EC_ERR_ALIGN_ARGUMENT: i32 = 221;

// Syncing — moving a Layout's work to and from the Vault it tracks

/// `exit_codes.sync`
pub const EC_ERR_SYNC: i32 = 230;

/// `exit_codes.sync_argument`
pub const EC_ERR_SYNC_ARGUMENT: i32 = 231;

// Checking in — bringing what the Vault holds into the Layout being worked in

/// `exit_codes.checkin`
pub const EC_ERR_CHECKIN: i32 = 240;

/// `exit_codes.checkin_argument`
pub const EC_ERR_CHECKIN_ARGUMENT: i32 = 241;
