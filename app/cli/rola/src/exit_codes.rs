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

// Exit code indicating `--install` could not find the user's key directory
pub const EC_ERR_TOOL_KEYGEN_NO_KEY_DIR: i32 = 44;

// Exit code indicating the directory a key pair was to be installed into could not be
// created
pub const EC_ERR_TOOL_KEYGEN_INSTALL_FAILED: i32 = 45;

// Exit code indicating the target or the input a handshake needs is missing
pub const EC_ERR_TOOL_HANDSHAKE_ARGUMENT: i32 = 46;

// Exit code indicating no account could be named to act as
pub const EC_ERR_TOOL_HANDSHAKE_NO_ACCOUNT: i32 = 47;

// Configuration

// Exit code indicating the configuration the run works from could not be read
pub const EC_ERR_CONFIG_UNREADABLE: i32 = 51;

// Vaults

// Exit code indicating a `rola vault` command was given arguments it cannot use
pub const EC_ERR_VAULT_ARGUMENT: i32 = 52;

// Exit code indicating the name a `rola vault unbind` was given is not bound
pub const EC_ERR_VAULT_NOT_BOUND: i32 = 53;

// Accounts

// Exit code indicating the name given is not an account the work can act as
pub const EC_ERR_ACCOUNT_NOT_FOUND: i32 = 61;

// Exit code indicating the machine does not name where Rorolala keeps the user's files
pub const EC_ERR_ACCOUNT_NO_DIR: i32 = 62;

// Placement

// Exit code indicating the run is not inside a Workspace, which the command needs
pub const EC_ERR_SHOULD_IN_WORKSPACE: i32 = 71;

// Exit code indicating the run is not inside a Vault, which the command needs
pub const EC_ERR_SHOULD_IN_VAULT: i32 = 72;

// Actions

// Exit code indicating an action had no channel to exchange over
pub const EC_ERR_ACTION_NO_CHANNEL: i32 = 81;

// Exit code indicating the side that owned a value held none to send
pub const EC_ERR_ACTION_MISSING_VALUE: i32 = 82;

// Exit code indicating a value was too long to frame
pub const EC_ERR_ACTION_VALUE_TOO_LARGE: i32 = 83;

// Exit code indicating the channel failed
pub const EC_ERR_ACTION_IO: i32 = 84;

// Exit code indicating a value could not cross the channel
pub const EC_ERR_ACTION_CODEC: i32 = 85;

// Exit code indicating no action answers to the id asked for
pub const EC_ERR_ACTION_UNKNOWN: i32 = 86;

// Exit code indicating what an action produced would not encode
pub const EC_ERR_ACTION_JSON: i32 = 87;

// Exit code indicating a target would not read as an address
pub const EC_ERR_ACTION_ADDR: i32 = 88;

// Exit code indicating a session could not be established
pub const EC_ERR_ACTION_AUTH: i32 = 89;
