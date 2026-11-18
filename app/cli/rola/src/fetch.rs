//! Content a run brings from the Vaults it can reach, at the moment it is needed.
//!
//! The exchange itself lives in `librorolala::fetch`; this keeps the command line's view of who may
//! be asked and how the run says what came back.

use librorolala::auth::Account;
use librorolala::fetch::Sources as Inner;
use librorolala::protocol::VaultAddress;
use librorolala::storage::{Key, RorolalaStorage};
use librorolala::workspace::Workspace;
use rorolala_cli_setups::ResCurrentRemoteVault;
use rorolala_utils_cli_theme::warn_line;
use rorolala_utils_progress::Progress;
use rust_i18n::t;

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

/// The Vaults a run may bring content from, and what it has to say about doing so.
pub struct Sources<'a> {
    inner: Inner<'a>,
}

impl<'a> Sources<'a> {
    /// The Vaults a run may ask: `primary` first, then every other one the Workspace has bound.
    ///
    /// # Errors
    ///
    /// Returns a message when the runtime the exchanges are waited on could not be made.
    pub fn new(
        workspace: &'a Workspace,
        account: &'a Account,
        remote: &ResCurrentRemoteVault,
        primary: Option<(String, VaultAddress)>,
        offline: bool,
    ) -> Result<Self, String> {
        Ok(Self {
            inner: Inner::new(workspace, account, candidates(remote, primary), offline)?,
        })
    }

    /// Spreads a transfer of many keys over this many reads at once.
    #[must_use]
    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.inner = self.inner.with_concurrency(concurrency);
        self
    }

    /// Says what the exchanges are doing while they run.
    #[must_use]
    pub fn reporting(mut self, progress: Progress) -> Self {
        self.inner = self.inner.reporting(progress);
        self
    }

    /// Brings `keys` from whichever Vault holds them, answering what is still missing.
    pub fn bring(&self, store: &RorolalaStorage, keys: &[Key]) -> Vec<Key> {
        self.inner.bring(store, keys)
    }

    /// Sends `keys` to the Vault the run is speaking to.
    ///
    /// # Errors
    ///
    /// Returns the exchange's own failure as a message.
    pub fn send(&self, store: Option<&RorolalaStorage>, keys: &[Key]) -> Result<usize, String> {
        self.inner.send(store, keys)
    }

    /// Whether there is a Vault to ask at all.
    #[must_use]
    pub fn can_ask(&self) -> bool {
        self.inner.can_ask()
    }

    /// Says what was brought, in one line, once the run is done with it.
    pub fn report(&self) {
        let took = self.inner.took();
        if took.is_empty() {
            return;
        }

        let count: usize = took.iter().map(|(_, count)| count).sum();
        let vaults: Vec<&str> = took.iter().map(|(name, _)| name.as_str()).collect();
        eprintln!(
            "{}",
            warn_line!(t!("fetch.brought", count = count, vaults = vaults.join(", ")).trim())
        );
    }
}
