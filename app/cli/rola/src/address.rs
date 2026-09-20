//! The addresses this program has been told to reach, remembered for completion.
//!
//! Nothing here is authoritative: it is a convenience so that the next `rola vault bind`
//! can offer an address that was used before instead of making it be typed out again.
//! Reading it is best-effort — a history that cannot be read is simply empty — and it is
//! kept under the user's local data directory, one address per line, sorted and without
//! repeats.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use mingling::{LazyInit, ProgramCollect, setup::ProgramSetup};

use crate::user::history_path;

/// The addresses this program has been told to reach.
///
/// The addresses are held as a set, so the same one is never remembered twice and the
/// order they are offered in is stable rather than the order they were seen in.
#[derive(Debug, Default, Clone)]
pub struct ResAddressHistory {
    /// Each address, in order.
    addresses: BTreeSet<String>,
}

impl ResAddressHistory {
    /// Remembers `address`, so a later completion can offer it.
    pub fn remember(&mut self, address: impl Into<String>) {
        self.addresses.insert(address.into());
    }

    /// Each address remembered, in order.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.addresses.iter().map(String::as_str)
    }

    /// Reads the history from `path`.
    ///
    /// What cannot be read is not an error: a history that is missing, or that cannot be
    /// read at all, comes back empty, since there is nothing here that a failure could be
    /// usefully reported against.
    fn read(path: &Path) -> Self {
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };

        Self {
            addresses: text
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect(),
        }
    }

    /// Writes the history to `path`, one address per line.
    ///
    /// The directory is made if it is not there yet. A failure is silent for the reason
    /// reading one is: remembering an address is a convenience and has no caller to
    /// report to.
    fn write(&self, path: &Path) {
        let Some(directory) = path.parent() else {
            return;
        };

        if fs::create_dir_all(directory).is_err() {
            return;
        }

        // A trailing newline makes the file read as a list of lines rather than one long
        // line, which is what `read` and anything else that opens it expect.
        let mut text = self
            .addresses
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            text.push('\n');
        }

        let _ = fs::write(path, text);
    }
}

/// Registers the address history, so completions can offer what was reached before.
///
/// This is Rorolala's own and is not part of the shared setups: the history is a
/// convenience of the `rola` program, not something every program that links the setups
/// wants to keep.
pub struct AddressHistorySetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for AddressHistorySetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        let path = history_path();
        let read_path = path.clone();
        let write_path = path;

        program.with_resource(
            ResAddressHistory::lazy_init(move || {
                read_path
                    .as_deref()
                    .map_or_else(ResAddressHistory::default, ResAddressHistory::read)
            })
            .with_on_drop(move |history: ResAddressHistory| {
                if let Some(path) = write_path.as_deref() {
                    history.write(path);
                }
            }),
        );
    }
}
