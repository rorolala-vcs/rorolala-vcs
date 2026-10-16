//! Content a run brings from the Vaults it can reach, at the moment it is needed.
//!
//! A Layout names versions, and a version is only a meaning once the store holds what it stored.
//! Which is what a run that writes a version's content back — a pull, a restore, a checkin — has to
//! have in front of it, and what it should not pay for until then: a Vault is reached for one
//! object that is not here, not for a whole store that mostly is.
//!
//! Bringing one object is [`action_sync_hashes`](librorolala::daemon::action_sync_hashes_async),
//! which carries the named keys both ways and asks nothing about the rest. Which Vault to ask is
//! not fixed: every Vault the Workspace has bound is a candidate, and since a key is the hash of its
//! content, any of them holding it hands back the same object. The one the run named is asked first,
//! and what it cannot cover is asked of the others.
//!
//! Asking costs a connection, so the asking is planned: each candidate is asked *what it holds* once
//! — [`action_list_remote`](librorolala::daemon::action_list_remote_async), the same listing
//! `storage ls-remote-storaged` prints — and what is left is taken from whichever candidate covers
//! the most of it, so the fewest Vaults are reached. A candidate that cannot be reached, or that
//! answers with none of what is left, is passed over rather than waited on.
//!
//! What was brought is said once, on stderr, when the run is done with it: a fetch a run did not
//! make is one it does not report, and an offline run makes none at all.

use std::cell::RefCell;
use std::collections::HashSet;

use librorolala::auth::Account;
use librorolala::daemon::{action_list_remote_async, action_sync_hashes_async};
use librorolala::protocol::VaultAddress;
use librorolala::storage::{Key, RorolalaStorage, StorageBackend as _};
use librorolala::workspace::Workspace;
use rorolala_cli_setups::ResCurrentRemoteVault;
use rorolala_utils_cli_theme::warn_line;
use rorolala_utils_progress::Progress;
use rust_i18n::t;

use crate::storage::cmd_storage_ls_remote_storaged::keys_of;

/// The Vault a run reaches for first: the one the Layout tracks, or the Workspace's own choice.
///
/// A Layout that tracks a Vault names the one its content belongs to, so that Vault is asked before
/// any other. One that tracks none is one whose content may be anywhere the Workspace has bound, and
/// the Workspace's own choice is then the first to be asked. `None` is a run with nowhere to ask,
/// which is a run that fetches nothing rather than one that fails.
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

/// The Vaults a run may bring content from, and what it has to say about doing so.
///
/// What the fetches come to is kept behind a [`RefCell`], since a run consults the same sources from
/// wherever it happens to read content — a restore writes many files through one borrow, a sync
/// reads one object per entry — and the asking is the same cache to all of them.
pub struct Sources<'a> {
    /// The Workspace the exchange is spoken from.
    workspace: &'a Workspace,
    /// The account the exchange is spoken as.
    account: &'a Account,
    /// Whether the run may reach a Vault at all.
    offline: bool,
    /// The Vaults to ask, the one the run named first.
    candidates: Vec<Candidate>,
    /// What asking them has come to.
    asked: RefCell<Vec<Asked>>,
    /// What the exchanges say about themselves while they run, which is nothing unless a caller
    /// hands one in — see [`reporting`](Sources::reporting).
    progress: Progress,
    /// The runtime the exchanges are waited on: a command is not asynchronous, and this one is made
    /// where the fetches are rather than borrowed, since a command that fetches has no other.
    runtime: tokio::runtime::Runtime,
    /// How many connections a transfer of many keys is spread over, which is never less than one.
    concurrency: usize,
}

/// One Vault a run may ask.
struct Candidate {
    /// The name the Workspace knows it by, for what the run says about the fetch.
    name: String,
    /// Where it answers.
    address: VaultAddress,
}

/// What asking one candidate has come to.
#[derive(Default, Clone)]
struct Asked {
    /// What it was found to hold: `None` until it is asked, and what its listing said after — an
    /// empty set for a Vault that could not be reached, which is a Vault holding nothing as far as
    /// this run is concerned.
    held: Option<HashSet<Key>>,
    /// How many lanes it granted, once it has been asked: `None` until the run asks.
    granted: Option<usize>,
    /// Whether what is left has already been asked of it, so it is not reached again.
    used: bool,
    /// How many objects it actually handed over, for the one line the run says.
    brought: usize,
}

impl<'a> Sources<'a> {
    /// The Vaults a run may ask: `primary` first, then every other one the Workspace has bound.
    ///
    /// The other Vaults are taken in name order, since the Workspace keeps them in no order at all
    /// and two runs asking the same question should reach them in the same sequence.
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
        let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;

        let mut candidates = Vec::new();
        if let Some((name, address)) = primary {
            candidates.push(Candidate { name, address });
        }

        let first = candidates.first().map(|candidate| candidate.name.clone());
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

            candidates.push(Candidate { name, address });
        }

        let asked = vec![Asked::default(); candidates.len()];

        Ok(Self {
            workspace,
            account,
            offline,
            candidates,
            asked: RefCell::new(asked),
            progress: Progress::silent(),
            runtime,
            concurrency: 1,
        })
    }

    /// Spreads a transfer of many keys over this many connections.
    ///
    /// A run that names none keeps the one connection a fetch has always used; `--concurrency` is
    /// what this is handed.
    #[must_use]
    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency.max(1);

        self
    }

    /// Hands in the progress the exchanges say themselves to, so a watched run shows a bar while
    /// content moves.
    ///
    /// A run that is not being watched hands nothing, and the exchanges say nothing: a transfer of
    /// many keys is worth a bar, and leaving it silent is what a run that asked for no bars gets.
    #[must_use]
    pub fn reporting(mut self, progress: Progress) -> Self {
        self.progress = progress;

        self
    }

    /// Brings `keys` here from whichever Vaults hold them, so a read that wants one finds it.
    ///
    /// A key that is already here is not asked about, and nothing at all is asked when the run is
    /// offline: what is missing stays missing, and the read that wanted it fails in its own words.
    /// A Vault that cannot be reached is passed over, so a fetch that cannot be made is not a
    /// failure of the run — it is one more thing that was not brought.
    pub fn bring(&self, store: &RorolalaStorage, keys: &[Key]) {
        if self.offline || self.candidates.is_empty() || keys.is_empty() {
            return;
        }

        let mut left = self.missing(store, keys);
        if left.is_empty() {
            return;
        }

        // Ask the candidates in order, and stop asking as soon as what has been seen covers what is
        // left: a Vault that is never asked is a Vault never reached.
        for index in 0..self.candidates.len() {
            if self.covered(&left) {
                break;
            }

            self.ask(index);
        }

        // Take what is left from whichever candidate covers the most of it, and ask again with what
        // is still missing: a candidate that answered with nothing is one more that was no use.
        while !left.is_empty() {
            let choice = {
                let asked = self.asked.borrow();

                chooser(&asked, &left)
            };
            let Some((index, wanted)) = choice else {
                break;
            };
            let before = left.len();

            self.carry(store, index, &wanted);
            left = self.missing(store, &left);
            self.asked.borrow_mut()[index].brought += before - left.len();
        }
    }

    /// Sends `keys` to the Vault the run is speaking to.
    ///
    /// This is the one direction with nowhere to fall back to: a version the Vault is to name has to
    /// be in the Vault's own store, so content that cannot be sent there is a failure of the run
    /// rather than something another Vault could stand in for.
    ///
    /// # Errors
    ///
    /// Returns the exchange's own failure as a message.
    pub fn send(&self, store: Option<&RorolalaStorage>, keys: &[Key]) -> Result<(), String> {
        let Some(primary) = self.candidates.first() else {
            return Ok(());
        };
        if keys.is_empty() {
            return Ok(());
        }

        // What is named is exploded into units here, before the keys are split: a content kept as
        // chunks becomes its manifest and each chunk, so the chunks travel over different lanes
        // rather than one behind another. What a run asked for is what it asks with: the units are
        // split into as many runs as there are lanes, and each travels over a connection of its own,
        // so the Vault serves them at once.
        let lines = self.unit_lines(store, keys);
        let ordered = primary.address.to_string();
        let width = self.granted(0);
        let results = self.runtime.block_on(async {
            futures::future::join_all(Self::lane_inputs(&lines, width).into_iter().map(|input| {
                action_sync_hashes_async(
                    self.workspace,
                    self.account,
                    ordered.clone(),
                    input,
                    self.progress.clone(),
                )
            }))
            .await
        });

        results
            .into_iter()
            .find_map(Result::err)
            .map_or(Ok(()), |error| Err(error.to_string()))
    }

    /// How many lanes a Vault grants, asking it once and remembering the answer.
    ///
    /// A Vault is asked for as many lanes as the run wants and answers no more than it serves, so
    /// the run opens no more than that. A Vault that could not be answered grants what was asked
    /// for, since the failure to reach it is said where the transfer is.
    fn granted(&self, index: usize) -> usize {
        if let Some(granted) = self.asked.borrow()[index].granted {
            return granted;
        }

        let address = self.candidates[index].address.clone();
        let requested = self.concurrency;
        let granted = self
            .runtime
            .block_on(action_sync_hashes_async(
                self.workspace,
                self.account,
                address.to_string(),
                format!("c:{requested}"),
                self.progress.clone(),
            ))
            .ok()
            .and_then(|answer| answer.trim().parse::<usize>().ok())
            .map_or(requested, |granted| granted.clamp(1, requested.max(1)));

        self.asked.borrow_mut()[index].granted = Some(granted);

        granted
    }

    /// The tokens `keys` are sent as: `m:` before a content kept as chunks, then `o:` before each
    /// chunk it names, and the key itself when the content is one object.
    ///
    /// A store is what says which of the two a key is; a run without one sends every key whole,
    /// which the Vault expands itself.
    fn unit_lines(&self, store: Option<&RorolalaStorage>, keys: &[Key]) -> Vec<String> {
        let Some(store) = store else {
            return keys.iter().map(Key::hex).collect();
        };

        let mut lines = Vec::new();
        for key in keys {
            match self.runtime.block_on(store.manifest_of(key)) {
                Ok(Some(manifest)) => {
                    lines.push(format!("m:{}", key.hex()));
                    for chunk in manifest.chunks() {
                        lines.push(format!("o:{}", chunk.key().hex()));
                    }
                }
                _ => lines.push(key.hex()),
            }
        }

        lines
    }

    /// `tokens` split into as many runs as the run carries at once, each joined as the exchange
    /// reads it: one unit a line, as a listing prints its keys.
    fn lane_inputs(tokens: &[String], width: usize) -> Vec<String> {
        if tokens.is_empty() {
            return Vec::new();
        }

        let count = width.clamp(1, tokens.len());
        tokens
            .chunks(tokens.len().div_ceil(count))
            .map(|chunk| chunk.join("\n"))
            .collect()
    }

    /// Says what was brought, in one line, once the run is done with it.
    ///
    /// Nothing is said when nothing was brought: a run that needed no fetch is a run with nothing to
    /// report, and an offline run is one that never looked.
    pub fn report(&self) {
        let asked = self.asked.borrow();
        let said: Vec<String> = self
            .candidates
            .iter()
            .zip(asked.iter())
            .filter(|(_, asked)| asked.brought > 0)
            .map(|(candidate, asked)| format!("{} {}", candidate.name, asked.brought))
            .collect();

        if said.is_empty() {
            return;
        }

        let count: usize = asked.iter().map(|asked| asked.brought).sum();
        eprintln!(
            "{}",
            warn_line!(t!("fetch.brought", count = count, vaults = said.join(", ")).trim())
        );
    }

    /// Which of `keys` are not here.
    ///
    /// A store that cannot answer is read as holding none of them: what is asked of a Vault and
    /// turns out to be here anyway costs a look-up and nothing else, while the other way round would
    /// leave content that is needed behind.
    fn missing(&self, store: &RorolalaStorage, keys: &[Key]) -> Vec<Key> {
        let Ok(presence) = self.runtime.block_on(store.contains_keys(keys)) else {
            return keys.to_vec();
        };

        keys.iter()
            .enumerate()
            .filter(|(index, _)| !presence.held(*index))
            .map(|(_, key)| *key)
            .collect()
    }

    /// Asks a candidate what it holds, once.
    fn ask(&self, index: usize) {
        if self.asked.borrow()[index].held.is_some() {
            return;
        }

        let address = self.candidates[index].address.clone();
        let listing = self.runtime.block_on(action_list_remote_async(
            self.workspace,
            self.account,
            address.to_string(),
            String::new(),
            self.progress.clone(),
        ));

        self.asked.borrow_mut()[index].held = Some(listing.map_or_else(
            |_| HashSet::new(),
            |text| keys_of(&text).into_iter().collect(),
        ));
    }

    /// Whether what has been seen covers every key still left.
    ///
    /// What is asked about is the Vaults *together*: one of them covering a key and another the
    /// next is the whole of what is missing being covered, and asking further Vaults would be
    /// reaching for what is already accounted for.
    fn covered(&self, left: &[Key]) -> bool {
        let asked = self.asked.borrow();

        left.iter().all(|key| {
            asked
                .iter()
                .filter_map(|asked| asked.held.as_ref())
                .any(|held| held.contains(key))
        })
    }

    /// Asks a candidate for the named keys, and marks it as used.
    ///
    /// A failure carries no further than the next candidate: what did not arrive is still missing,
    /// and the loop asks whichever candidate is left.
    fn carry(&self, store: &RorolalaStorage, index: usize, keys: &[Key]) {
        self.asked.borrow_mut()[index].used = true;

        let address = self.candidates[index].address.to_string();
        let width = self.granted(index);

        // What is asked for first is whichever of a manifest or an object the Vault holds: a content
        // kept as chunks answers with its manifest, which says what to ask for next, and one kept
        // whole answers with the object and is done.
        let discovered: Vec<String> = keys.iter().map(|key| format!("q:{}", key.hex())).collect();
        self.transfer(&address, &discovered, width);

        // What arrived kept as chunks is asked for a chunk at a time, the chunks of one content
        // spread over the lanes: the manifest the first pass brought is what says which they are.
        let mut chunks = Vec::new();
        for key in keys {
            if let Ok(Some(manifest)) = self.runtime.block_on(store.manifest_of(key)) {
                for chunk in manifest.chunks() {
                    chunks.push(format!("o:{}", chunk.key().hex()));
                }
            }
        }

        self.transfer(&address, &chunks, width);
    }

    /// Makes the Vault hold everything here and this side hold everything there, in units spread over
    /// as many lanes as it granted.
    fn transfer(&self, address: &str, tokens: &[String], width: usize) {
        if tokens.is_empty() {
            return;
        }

        let inputs = Self::lane_inputs(tokens, width);
        let _ = self.runtime.block_on(async {
            futures::future::join_all(inputs.iter().map(|input| {
                action_sync_hashes_async(
                    self.workspace,
                    self.account,
                    address.to_owned(),
                    input.clone(),
                    self.progress.clone(),
                )
            }))
            .await
        });
    }
}

/// The candidate to ask next, and what to ask it for.
///
/// The one covering the most of what is left is the one reached, so a run that needs three objects
/// two Vaults hold ends by visiting one of them rather than both. The first of equals wins, which is
/// the Vault the run named before the ones it did not: the order candidates are kept in is the order
/// they are tried. A candidate that was already asked, or that covers none of what is left, is not
/// chosen while any other can be.
fn chooser(asked: &[Asked], left: &[Key]) -> Option<(usize, Vec<Key>)> {
    let mut best: Option<(usize, Vec<Key>)> = None;

    for (index, asked) in asked.iter().enumerate() {
        if asked.used {
            continue;
        }

        let Some(held) = asked.held.as_ref() else {
            continue;
        };
        let keys: Vec<Key> = left
            .iter()
            .filter(|key| held.contains(key))
            .copied()
            .collect();
        let better = match &best {
            None => true,
            Some((_, most)) => keys.len() > most.len(),
        };

        if !keys.is_empty() && better {
            best = Some((index, keys));
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A key that is told apart from another by its digest alone.
    fn key(seed: u8) -> Key {
        Key::new([seed; 32])
    }

    /// The units are split into as many lanes as the run carries at once, one whole run per lane and
    /// none of them empty.
    #[test]
    fn the_units_are_split_into_as_many_lanes_as_asked() {
        let tokens: Vec<String> = (0..10).map(|at| at.to_string()).collect();

        let four = Sources::lane_inputs(&tokens, 4);
        assert_eq!(four.len(), 4);
        assert_eq!(
            four.iter().map(|lane| lane.lines().count()).sum::<usize>(),
            10
        );
        assert!(four.iter().all(|lane| !lane.is_empty()));

        // More lanes than there are units is one lane a unit, and one lane is every unit at once.
        assert_eq!(Sources::lane_inputs(&tokens, 100).len(), 10);
        assert_eq!(Sources::lane_inputs(&tokens, 1).len(), 1);
        assert_eq!(Sources::lane_inputs(&[], 4), Vec::<String>::new());
    }

    /// A candidate that was asked and answered with `keys`.
    fn holds(keys: &[u8]) -> Asked {
        Asked {
            held: Some(keys.iter().copied().map(key).collect()),
            granted: None,
            used: false,
            brought: 0,
        }
    }

    /// The candidate that covers the most of what is left is the one asked.
    #[test]
    fn the_candidate_covering_the_most_is_chosen() {
        let asked = [holds(&[1]), holds(&[1, 2])];
        let (index, wanted) = chooser(&asked, &[key(1), key(2)]).unwrap();

        assert_eq!(index, 1);
        assert_eq!(wanted, vec![key(1), key(2)]);
    }

    /// Two candidates that cover as much are told apart by the order they are tried in, which is
    /// the one the run named first.
    #[test]
    fn an_equal_choice_goes_to_the_first() {
        let asked = [holds(&[1, 2]), holds(&[1, 2])];
        let (index, _) = chooser(&asked, &[key(1), key(2)]).unwrap();

        assert_eq!(index, 0);
    }

    /// What a candidate that was already asked covered is not asked of it again.
    #[test]
    fn a_candidate_already_asked_is_passed_over() {
        let asked = [
            Asked {
                held: Some(std::iter::once(key(1)).collect()),
                granted: None,
                used: true,
                brought: 1,
            },
            holds(&[2]),
        ];
        let (index, _) = chooser(&asked, &[key(1), key(2)]).unwrap();

        assert_eq!(index, 1);
    }

    /// A candidate that covers none of what is left is not chosen while another covers any of it,
    /// and nothing is chosen when none of them does.
    #[test]
    fn nothing_is_chosen_when_nothing_covers() {
        let asked = [holds(&[3]), holds(&[4])];

        assert!(chooser(&asked, &[key(1)]).is_none());
    }

    /// A candidate that was never asked has nothing to say.
    #[test]
    fn a_candidate_never_asked_is_not_chosen() {
        let asked = [
            Asked {
                held: None,
                granted: None,
                used: false,
                brought: 0,
            },
            holds(&[1]),
        ];
        let (index, _) = chooser(&asked, &[key(1)]).unwrap();

        assert_eq!(index, 1);
    }
}
