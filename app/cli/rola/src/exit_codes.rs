// Exit code indicating the resource already exists
pub const EC_ALREADY_EXIST: i32 = 11;

// Exit code indicating the resource does not exist
pub const EC_NOT_EXIST: i32 = 12;

// Exit code indicating help was triggered
pub const EC_HELP: i32 = 2;

// Creation

// Exit code indicating failure to create a workspace
pub const EC_ERR_CREATION_WORKSPACE: i32 = 21;

// Exit code indicating failure to create a vault
pub const EC_ERR_CREATION_VAULT: i32 = 22;

// Tools

// Exit code indicating `openssl` could not be run
pub const EC_ERR_TOOL_KEYGEN_NO_OPENSSL: i32 = 41;

// Exit code indicating `openssl` ran but did not produce a key
pub const EC_ERR_TOOL_KEYGEN_FAILED: i32 = 42;

// Exit code indicating the path a key was to be written to is not there
pub const EC_ERR_TOOL_KEYGEN_PATH_NOT_EXIST: i32 = 43;
