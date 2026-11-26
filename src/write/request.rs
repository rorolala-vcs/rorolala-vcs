//! What a write is asked for, as a neutral value the command line and the C ABI both fill.
//!
//! A request names the Workspace, the account and the Vault by value rather than by the resources a
//! command happens to hold, so the same request can be built from arguments and from JSON. Resolving
//! it is one reading of the Workspace's configuration and its keys, which is why it lives here and
//! not in either caller: a second reading would be a second set of rules to keep right.

use std::path::PathBuf;

use rorolala_auth::Account;
use rorolala_layout::Layout;
use rorolala_protocol::VaultAddress;
use rorolala_storage::StorageSet;
use rorolala_utils_configure::Configure as _;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vcs::VCSIndex;
use rorolala_workspace::{Config as WorkspaceConfig, Workspace};
use serde::Deserialize;

use crate::keys;
use crate::remote::{self, ErrorRemoteVault};
use crate::write::{Labels, WriteError};

/// Which Vault a write reaches for.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    /// The Vault the Layout being worked in tracks.
    #[default]
    Tracked,
    /// A Vault the Workspace knows, under this name.
    Named {
        /// The name the Workspace bound it to.
        name: String,
    },
    /// An address, which is a Vault the Workspace never named.
    Address {
        /// Where it answers.
        address: String,
    },
}

/// The fields every write request carries.
///
/// The defaults are the ones a run that said nothing means: no account named, the tracked Vault, a
/// run that may reach a Vault, and English words for the progress it says.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Common {
    /// The Workspace the write is made in, by its root.
    pub workspace: PathBuf,
    /// The account the write acts as, by name.
    #[serde(default)]
    pub account: String,
    /// The Vault it reaches for.
    #[serde(default)]
    pub target: Target,
    /// Whether the run may reach a Vault at all.
    #[serde(default)]
    pub offline: bool,
    /// Whether the run is told to go past a gate.
    #[serde(default)]
    pub force: bool,
    /// How many reads a transfer keeps in flight.
    #[serde(default)]
    pub concurrency: usize,
    /// What the write calls the work while it does it.
    #[serde(default)]
    pub labels: Labels,
}

/// The Vault a request resolved to.
///
/// It is re-exported here so a caller of the write workflows keeps reading it under the path
/// the request is written in, while there is one definition of it in [`crate::remote`].
pub use crate::remote::Reached;

/// The Vault a request reaches for, as a C caller names it.
///
/// It mirrors [`Target`] field for field: a unit variant names the tracked Vault, and the other
/// two carry the name or the address they reach for.
///
/// # FFI
///
/// A `RolaWriteTarget` the caller fills: `tag` selects the variant and the payload companion it
/// names holds that variant's string. The string is borrowed for the call and released by the
/// caller, so nothing here is freed when the call returns.
#[lazyffi(export = RolaWriteTarget)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum WriteTarget {
    /// The Vault the Layout being worked in tracks.
    #[default]
    Tracked,
    /// A Vault the Workspace knows, under this name.
    Named {
        /// The name the Workspace bound it to.
        name: String,
    },
    /// An address, which is a Vault the Workspace never named.
    Address {
        /// Where it answers.
        address: String,
    },
}

impl From<WriteTarget> for Target {
    fn from(target: WriteTarget) -> Self {
        match target {
            WriteTarget::Tracked => Self::Tracked,
            WriteTarget::Named { name } => Self::Named { name },
            WriteTarget::Address { address } => Self::Address { address },
        }
    }
}

/// The words a write uses while it runs, as a C caller fills them.
///
/// # FFI
///
/// A `RolaWriteLabels` the caller fills; every `const char *` is borrowed for the call. A field
/// left as the empty string keeps the native word, so a caller translates only what it shows.
#[lazyffi(export = RolaWriteLabels, input)]
#[derive(Debug, Clone, Default)]
pub struct WriteLabels {
    /// Reading the tree, a Layout or an index.
    pub reading: String,
    /// Writing content into the store.
    pub writing: String,
    /// Reaching the Vault for a Layout or an object.
    pub fetching: String,
    /// Carrying content to the Vault.
    pub uploading: String,
    /// Carrying content from the Vault.
    pub downloading: String,
    /// Checking the work before writing it.
    pub checking: String,
    /// Carrying out a plan.
    pub applying: String,
}

impl From<WriteLabels> for Labels {
    fn from(input: WriteLabels) -> Self {
        let default = Self::default();

        Self {
            reading: pick(input.reading, default.reading),
            writing: pick(input.writing, default.writing),
            fetching: pick(input.fetching, default.fetching),
            uploading: pick(input.uploading, default.uploading),
            downloading: pick(input.downloading, default.downloading),
            checking: pick(input.checking, default.checking),
            applying: pick(input.applying, default.applying),
        }
    }
}

/// The fields every write request carries, as a C caller fills them.
///
/// It mirrors [`Common`] field for field. The Workspace is a path rather than a [`PathBuf`], so a
/// C caller writes one string.
///
/// # FFI
///
/// A `RolaWriteCommon` the caller fills beside a request's own fields. Every `const char *` is
/// borrowed for the call and released by the caller, so nothing here is freed when the call
/// returns. A zeroed `concurrency` leaves the choice to the native side, and an empty `account`
/// names none.
#[lazyffi(export = RolaWriteCommon, input)]
#[derive(Debug, Clone, Default)]
pub struct WriteCommon {
    /// The Workspace the write is made in, by its root.
    pub workspace: String,
    /// The account the write acts as, by name, empty for none.
    pub account: String,
    /// The Vault it reaches for.
    pub target: WriteTarget,
    /// Whether the run may reach a Vault at all.
    pub offline: bool,
    /// Whether the run is told to go past a gate.
    pub force: bool,
    /// How many reads a transfer keeps in flight, zero for the native side's own choice.
    pub concurrency: usize,
    /// What the write calls the work while it does it.
    pub labels: WriteLabels,
}

impl From<WriteCommon> for Common {
    fn from(input: WriteCommon) -> Self {
        Self {
            workspace: PathBuf::from(input.workspace),
            account: input.account,
            target: input.target.into(),
            offline: input.offline,
            force: input.force,
            concurrency: input.concurrency,
            labels: input.labels.into(),
        }
    }
}

/// The word a field carries, or the native default when it names none.
fn pick(written: String, fallback: String) -> String {
    if written.is_empty() {
        fallback
    } else {
        written
    }
}

impl Common {
    /// The Workspace the request names.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::NoWorkspace`] when no Workspace holds the directory.
    pub fn workspace(&self) -> Result<Workspace, WriteError> {
        Workspace::locate(&self.workspace).ok_or(WriteError::NoWorkspace)
    }

    /// The Layout the request works in, which is the current one.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::NoLayout`] when the Workspace works in none, and
    /// [`WriteError::Layout`] when the current one cannot be read.
    pub fn layout(&self, workspace: &Workspace) -> Result<Layout, WriteError> {
        workspace
            .layout_if_there()
            .map_err(|error| WriteError::Layout {
                reason: error.to_string(),
            })?
            .ok_or(WriteError::NoLayout)
    }

    /// The account the request acts as.
    ///
    /// The scopes are the Workspace's keys first, then the user's own stores, which is the same
    /// search a command's completion offers from.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::NoAccount`] when no scope holds an account by that name.
    pub fn account(&self, workspace: &Workspace) -> Result<Account, WriteError> {
        if self.account.is_empty() {
            return Err(WriteError::NoAccount);
        }

        keys::account_named(&self.account, Some(workspace), None).ok_or(WriteError::NoAccount)
    }

    /// The Vault the request reaches for.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::NoTrack`] when the target is the tracked Vault and the Layout tracks
    /// none, [`WriteError::Precondition`] when the Workspace's configuration names no Vault, names
    /// a name that is not one, or holds an address that does not read, and [`WriteError::Layout`]
    /// when the tracked name cannot be read.
    pub fn vault(&self, workspace: &Workspace) -> Result<Reached, WriteError> {
        match &self.target {
            Target::Address { address } => Ok(Reached {
                name: String::new(),
                address: VaultAddress::parse(address).map_err(|_| WriteError::Precondition {
                    what: format!("the Vault address `{address}`"),
                    reason: "it does not read as an address".to_owned(),
                })?,
            }),
            Target::Tracked => {
                let track = workspace
                    .layout_track()
                    .map_err(|error| WriteError::Layout {
                        reason: error.to_string(),
                    })?
                    .ok_or(WriteError::NoTrack)?;

                Self::named(workspace, &track)
            }
            Target::Named { name } => Self::named(workspace, name),
        }
    }

    /// The Vault `named` names, when the Workspace knows it.
    fn named(workspace: &Workspace, named: &str) -> Result<Reached, WriteError> {
        let config = Self::config(workspace)?;

        let name = remote::name(named, &config).map_err(|error| match error {
            // Naming none chooses the Workspace's own, which is held to the same rules as a name
            // that was given: a choice is a choice, whoever made it.
            ErrorRemoteVault::NotChosen => WriteError::Precondition {
                what: "the Vault to reach for".to_owned(),
                reason: "the Workspace reaches for none, and none was named".to_owned(),
            },
            ErrorRemoteVault::NotBound { name } => {
                // The name was refused for one of two reasons, and a write says which: a name
                // the Workspace never bound is not the same mistake as one that names a path.
                if config.vaults().get(&name).is_none() {
                    WriteError::Precondition {
                        what: format!("the Vault `{name}`"),
                        reason: "the Workspace has not bound it".to_owned(),
                    }
                } else {
                    WriteError::Precondition {
                        what: format!("the Vault name `{name}`"),
                        reason: "it names a path rather than a name".to_owned(),
                    }
                }
            }
            // A request holds no Workspace state, so the ways a setup refuses a Vault before a
            // name is read cannot be reached here.
            _ => WriteError::Precondition {
                what: format!("the Vault `{named}`"),
                reason: "the Workspace could not answer which Vault it names".to_owned(),
            },
        })?;

        let address = remote::address(&name, &config).map_err(|error| match error {
            ErrorRemoteVault::NotAddress { name } => WriteError::Precondition {
                what: format!("the Vault address `{name}`"),
                reason: "it does not read as an address".to_owned(),
            },
            // The name is already chosen, so only the address it reaches for is left to refuse.
            _ => WriteError::Precondition {
                what: format!("the Vault `{name}`"),
                reason: "the Workspace could not answer where it answers".to_owned(),
            },
        })?;

        Ok(Reached { name, address })
    }

    /// Every Vault the Workspace knows, the one the run reaches for first.
    ///
    /// A key is the hash of what it names, so which Vault answers says nothing about the content:
    /// the order is the one the run reaches for, then the names sorted, so the same workspace asks
    /// the same question the same way twice.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Precondition`] when the Workspace's configuration cannot be read. An
    /// address that does not read is left out, since it is the configuration that is wrong rather
    /// than the request, and the other Vaults still answer.
    pub fn candidates(
        &self,
        workspace: &Workspace,
    ) -> Result<Vec<(String, VaultAddress)>, WriteError> {
        let config = Self::config(workspace)?;
        let mut names: Vec<&String> = config.vaults().names().collect();
        names.sort();

        let mut found: Vec<(String, VaultAddress)> = names
            .into_iter()
            .filter_map(|name| {
                let written = config.vaults().get(name)?;
                let address = VaultAddress::parse(written).ok()?;

                Some((name.clone(), address))
            })
            .collect();

        if let Ok(primary) = self.vault(workspace)
            && !primary.name.is_empty()
            && let Some(at) = found.iter().position(|(name, _)| *name == primary.name)
        {
            let first = found.remove(at);
            found.insert(0, first);
        }

        Ok(found)
    }

    /// The stores the Workspace works on.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::Storage`] when the Workspace's configuration is there and does not
    /// read.
    pub fn store(&self, workspace: &Workspace) -> Result<StorageSet, WriteError> {
        Ok(workspace.storage_set()?)
    }

    /// The index the Workspace keeps.
    ///
    /// # Errors
    ///
    /// Returns [`WriteError::NoIndex`] when no index is there.
    pub fn index(&self, workspace: &Workspace) -> Result<VCSIndex, WriteError> {
        VCSIndex::locate(workspace.get_root()).ok_or(WriteError::NoIndex)
    }

    /// The Workspace's configuration.
    fn config(workspace: &Workspace) -> Result<WorkspaceConfig, WriteError> {
        WorkspaceConfig::read_from(&workspace.config_path()).map_err(|error| {
            WriteError::Precondition {
                what: "the Workspace's configuration".to_owned(),
                reason: error.to_string(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rorolala_utils_location::Locate as _;
    use rorolala_workspace::Workspace;

    use super::{Common, Target, WriteLabels, WriteTarget};
    use crate::write::{Labels, WriteError};

    /// A Workspace that bound no Vault and tracks none has nothing to reach for.
    #[test]
    fn a_workspace_with_no_vault_reaches_for_none() {
        let dir = std::env::temp_dir().join(format!("rola-request-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Workspace::create(&dir).expect("a Workspace");

        let workspace = Workspace::locate(&dir).expect("the Workspace is there");
        let common = Common {
            workspace: dir.clone(),
            target: Target::Tracked,
            ..Common::default()
        };

        assert!(common.vault(&workspace).is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_write_target_becomes_the_request_target() {
        assert_eq!(Target::from(WriteTarget::Tracked), Target::Tracked);
        assert_eq!(
            Target::from(WriteTarget::Named {
                name: "studio".to_owned()
            }),
            Target::Named {
                name: "studio".to_owned()
            }
        );
        assert_eq!(
            Target::from(WriteTarget::Address {
                address: "127.0.0.1:1".to_owned()
            }),
            Target::Address {
                address: "127.0.0.1:1".to_owned()
            }
        );
    }

    #[test]
    fn labels_left_empty_keep_the_native_words() {
        let labels: Labels = WriteLabels {
            reading: "Reading the tree".to_owned(),
            ..WriteLabels::default()
        }
        .into();
        let default = Labels::default();

        assert_eq!(labels.reading, "Reading the tree");
        assert_eq!(labels.writing, default.writing);
        assert_eq!(labels.applying, default.applying);
    }

    #[test]
    fn an_address_target_is_parsed_or_refused() {
        let dir = std::env::temp_dir().join(format!("rola-cov-request-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Workspace::create(&dir).expect("a Workspace");
        let workspace = Workspace::locate(&dir).expect("the Workspace is there");

        let bad = Common {
            workspace: dir.clone(),
            target: Target::Address {
                address: "not an address".to_owned(),
            },
            ..Common::default()
        };
        assert!(matches!(
            bad.vault(&workspace),
            Err(WriteError::Precondition { .. })
        ));
        assert!(bad.layout(&workspace).is_ok());

        let good = Common {
            workspace: dir.clone(),
            target: Target::Address {
                address: "127.0.0.1:1".to_owned(),
            },
            ..Common::default()
        };
        let reached = good.vault(&workspace).expect("an address answers");
        assert_eq!(reached.name, "");
        assert_ne!(reached.address.to_string(), "");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_configuration_that_does_not_read_is_refused() {
        let dir =
            std::env::temp_dir().join(format!("rola-cov-request-config-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Workspace::create(&dir).expect("a Workspace");
        let workspace = Workspace::locate(&dir).expect("the Workspace is there");
        fs::write(workspace.config_path(), "not toml").expect("a broken configuration");

        let common = Common {
            workspace: dir.clone(),
            target: Target::Named {
                name: "studio".to_owned(),
            },
            ..Common::default()
        };
        assert!(matches!(
            common.vault(&workspace),
            Err(WriteError::Precondition { .. })
        ));
        assert!(common.candidates(&workspace).is_err());

        let _ = fs::remove_dir_all(&dir);
    }
}
