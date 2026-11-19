//! Content a run brings from the Vaults it can reach, at the moment it is needed.
//!
//! The exchange itself lives in `librorolala::fetch`; this keeps the command line's view of who may
//! be asked. What a fetch came to is said where the run that made it is, so nothing of it is kept
//! here.

use librorolala::protocol::VaultAddress;
use rorolala_cli_setups::ResCurrentRemoteVault;

/// The Vault a run reaches for first: the one the Layout tracks, or the Workspace's own choice.
#[must_use]
pub fn primary(
    remote: &ResCurrentRemoteVault,
    tracked: Option<&str>,
) -> Option<(String, VaultAddress)> {
    if let Some(tracked) = tracked {
        let address = remote.vault_or_default(tracked).ok()?;
        let name = remote
            .name_or_default(tracked)
            .unwrap_or_else(|_| tracked.to_owned());

        return Some((name, address));
    }

    let address = remote.vault().ok()?;
    let name = remote.name_or_default("").ok()?;

    Some((name, address))
}

/// The Vaults a run may bring content from: `primary` first, then every other one the Workspace has
/// bound.
#[must_use]
pub fn candidates(
    remote: &ResCurrentRemoteVault,
    primary: Option<(String, VaultAddress)>,
) -> Vec<(String, VaultAddress)> {
    let mut candidates: Vec<(String, VaultAddress)> = Vec::new();
    if let Some(primary) = primary {
        candidates.push(primary);
    }

    let first = candidates.first().map(|(name, _)| name.clone());
    let mut names: Vec<String> = remote
        .names()
        .unwrap_or_default()
        .into_iter()
        .map(str::to_owned)
        .filter(|name| Some(name) != first.as_ref())
        .collect();
    names.sort();

    for name in names {
        let Ok(address) = remote.vault_or_default(name.clone()) else {
            continue;
        };

        candidates.push((name, address));
    }

    candidates
}
