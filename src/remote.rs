//! Resolving the Vault a run reaches for from the Workspace beside it.
//!
//! A Workspace's configuration is read once, by whoever can say whether there was a Workspace
//! to read at all, and what that reading means is answered here. The command line and the C ABI
//! ask the same questions of the same file, so one reading of the rules keeps the two from
//! coming to different answers about which Vault a name reaches for.

use std::path::{Component, Path, PathBuf};

use rorolala_errors::Failure;
use rorolala_protocol::VaultAddress;
use rorolala_workspace::Config as WorkspaceConfig;

/// The Vault a request resolved to.
#[derive(Debug, Clone)]
pub struct Reached {
    /// The name the Workspace knows it by, which is empty for an address.
    pub name: String,
    /// Where it answers.
    pub address: VaultAddress,
}

/// The address the Vault `name` reaches for.
///
/// A name the Workspace knows says where to go, written down as the link it is; one it does not
/// is taken to be an address already, which is what lets a Vault be reached that was never given
/// a name. An empty `name` chooses the Vault the Workspace reaches for, so a caller that was
/// given nothing asks for the default without knowing whether one was chosen.
///
/// # Errors
///
/// Returns [`ErrorRemoteVault::NotChosen`] when `name` is empty and the Workspace reaches for no
/// Vault, and [`ErrorRemoteVault::NotAddress`] when what the name resolves to does not read as
/// an address.
pub fn address(name: &str, config: &WorkspaceConfig) -> Result<VaultAddress, ErrorRemoteVault> {
    let name = chosen(name, config)?;

    address_of(name, config)
}

/// The name of the Vault `name` reaches for.
///
/// It is [`address`] read for the name rather than the address, which is what a caller needs
/// when what it keeps is named after the Vault: a name the Workspace bound, and never an address,
/// since an address is where a Vault answers rather than what it is called. An empty `name`
/// chooses the Vault the Workspace reaches for.
///
/// # Errors
///
/// Returns [`ErrorRemoteVault::NotChosen`] when `name` is empty and the Workspace reaches for no
/// Vault, and [`ErrorRemoteVault::NotBound`] when a name was given that is not one the Workspace
/// has bound, or one that names a path rather than a single name and so cannot be kept under.
pub fn name(name: &str, config: &WorkspaceConfig) -> Result<String, ErrorRemoteVault> {
    let name = chosen(name, config)?;

    if config.vaults().get(name).is_none() || !is_plain_name(name) {
        return Err(ErrorRemoteVault::NotBound {
            name: name.to_owned(),
        });
    }

    Ok(name.to_owned())
}

/// Each Vault the Workspace knows, under the name it is known by.
///
/// These are the names a caller can reach for, so they are also the ones worth offering: the
/// same set [`address`] and [`name`] resolve.
#[must_use]
pub fn names(config: &WorkspaceConfig) -> Vec<&str> {
    config.vaults().names().map(String::as_str).collect()
}

/// The name a caller reaches for: the one given, or the Workspace's own when none was given.
fn chosen<'a>(name: &'a str, config: &'a WorkspaceConfig) -> Result<&'a str, ErrorRemoteVault> {
    if name.is_empty() {
        config
            .default_config()
            .vault()
            .ok_or(ErrorRemoteVault::NotChosen)
    } else {
        Ok(name)
    }
}

/// The address `name` names, read as one.
///
/// A name the Workspace knows says where to go, written down as the link it is; one it does not
/// is taken to be an address already, which is what lets a Vault be reached that was never given
/// a name. What is neither a name it knows nor a readable address is reported rather than handed
/// on: a configuration that was edited by hand can hold an address no caller could have written,
/// and there is nowhere either could be dialled.
fn address_of(name: &str, config: &WorkspaceConfig) -> Result<VaultAddress, ErrorRemoteVault> {
    let written = config.vaults().get(name).map_or(name, String::as_str);

    VaultAddress::parse(written).map_err(|_| ErrorRemoteVault::NotAddress {
        name: written.to_string(),
    })
}

/// Whether `name` is one path component and nothing else.
///
/// A Vault's name is kept under the Workspace's cache as a directory of its own, so a name that
/// names a path (or climbs, or is a drive) would be kept outside it, and a name that is not one
/// name is not one the Workspace could have meant as a Vault's own.
fn is_plain_name(name: &str) -> bool {
    let mut components = Path::new(name).components();

    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

/// Error: the Vault a run was to reach for could not be worked out.
///
/// The ways there can be none are told apart because what is to be done about them differs:
/// work inside a Workspace, fix the configuration the Workspace keeps, or choose a Vault for
/// it to reach for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorRemoteVault {
    /// This run is not inside a Workspace, so nothing could have named a Vault.
    ShouldInWorkspace,
    /// The Workspace's configuration is there, but would not read.
    Unread {
        /// The file that could not be read.
        path: PathBuf,
        /// Why it could not be read.
        reason: String,
    },
    /// The Workspace is here, and reaches for no Vault.
    NotChosen,
    /// The name is neither one the Workspace knows nor an address that reads.
    NotAddress {
        /// What was named.
        name: String,
    },
    /// The name is not one the Workspace has bound, and an address is not a name.
    NotBound {
        /// What was named.
        name: String,
    },
}

impl Failure for ErrorRemoteVault {
    /// The name of the way no Vault could be reached, as a program reads it.
    ///
    /// The ways are told apart because what is to be done about them differs: work inside a
    /// Workspace, fix the configuration the Workspace keeps, or choose a Vault for it to
    /// reach for.
    fn name(&self) -> &'static str {
        match self {
            Self::ShouldInWorkspace => "error_remote_vault_should_in_workspace",
            Self::Unread { .. } => "error_remote_vault_unread",
            Self::NotChosen => "error_remote_vault_not_chosen",
            Self::NotAddress { .. } => "error_remote_vault_not_address",
            Self::NotBound { .. } => "error_remote_vault_not_bound",
        }
    }

    /// What went wrong, in the library's own words, with what it went wrong about.
    ///
    /// What a person is shown is said where the command is, in the run's language. What is
    /// added here is the file or the name the failure is about, which is the same in any
    /// language.
    fn reason(&self) -> String {
        match self {
            Self::ShouldInWorkspace => {
                "this run is not inside a Workspace, so no Vault could have been named".to_owned()
            }
            Self::Unread { path, reason } => format!(
                "the Workspace configuration at {} could not be read: {reason}",
                path.display()
            ),
            Self::NotChosen => {
                "no Vault was named to reach for, and the Workspace reaches for none".to_owned()
            }
            Self::NotAddress { name } => {
                format!("`{name}` is not a Vault this Workspace knows, nor an address")
            }
            Self::NotBound { name } => {
                format!("`{name}` is not a Vault this Workspace has bound")
            }
        }
    }
}

rorolala_errors::failure!(ErrorRemoteVault);

#[cfg(test)]
mod tests {
    use rorolala_protocol::VaultAddress;
    use rorolala_workspace::Config as WorkspaceConfig;

    use super::{ErrorRemoteVault, address, is_plain_name, name};

    /// A Workspace that bound two Vaults and chose one of them.
    fn config() -> WorkspaceConfig {
        let mut config = WorkspaceConfig::default();
        config.vaults_mut().bind("origin", "127.0.0.1:9270");
        config
            .vaults_mut()
            .bind("upstream", "rola://example.test/studio");
        config.default_config_mut().set_vault("origin");

        config
    }

    #[test]
    fn a_bound_name_reaches_the_address_it_was_bound_to() {
        let config = config();
        let reached = address("origin", &config).expect("a bound name reaches an address");

        assert_eq!(
            reached,
            VaultAddress::parse("127.0.0.1:9270").expect("a written address")
        );
    }

    #[test]
    fn a_name_the_workspace_does_not_know_is_read_as_an_address() {
        let config = config();
        let reached = address("127.0.0.1:5000", &config).expect("an address reaches itself");

        assert_eq!(
            reached,
            VaultAddress::parse("127.0.0.1:5000").expect("a written address")
        );
    }

    #[test]
    fn naming_none_reaches_for_the_vault_the_workspace_chose() {
        let config = config();

        assert_eq!(
            address("", &config).expect("the Workspace chose one"),
            VaultAddress::parse("127.0.0.1:9270").expect("a written address")
        );
        assert_eq!(
            name("", &config).expect("the Workspace chose one"),
            "origin"
        );
    }

    #[test]
    fn a_workspace_that_chose_none_has_nothing_to_reach_for() {
        let mut config = config();
        config.default_config_mut().clear_vault();

        assert!(matches!(
            address("", &config),
            Err(ErrorRemoteVault::NotChosen)
        ));
        assert!(matches!(
            name("", &config),
            Err(ErrorRemoteVault::NotChosen)
        ));
    }

    #[test]
    fn a_name_that_is_not_an_address_is_reported_as_written() {
        let config = config();

        assert!(matches!(
            address("not an address", &config),
            Err(ErrorRemoteVault::NotAddress { name }) if name == "not an address"
        ));
    }

    #[test]
    fn a_name_the_workspace_did_not_bind_is_refused_as_a_name() {
        let config = config();

        assert!(matches!(
            name("nowhere", &config),
            Err(ErrorRemoteVault::NotBound { name }) if name == "nowhere"
        ));
        assert!(matches!(
            name("a/b", &config),
            Err(ErrorRemoteVault::NotBound { name }) if name == "a/b"
        ));
    }

    /// A Vault's name is kept as a directory under the Workspace's cache, so anything that names
    /// more than one component (or climbs, or is a drive) is not one.
    ///
    /// What is a separator is the platform's, so the spellings that are only separators somewhere
    /// are not listed here: on Windows `C:` is a prefix and `a\b` is two names, and on Unix both
    /// are single file names, which is what each platform's own `components` says.
    #[test]
    fn a_name_that_names_more_than_a_name_is_refused() {
        for good in ["origin", "origin-2", "a.b", "..a"] {
            assert!(is_plain_name(good), "{good}");
        }

        for bad in ["", ".", "..", "a/b", "/a", "a/../b", "C:/a"] {
            assert!(!is_plain_name(bad), "{bad}");
        }
    }
}
