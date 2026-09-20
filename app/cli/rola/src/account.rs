//! The account the work acts as, kept in Rorolala's own file for the user.
//!
//! It is kept the way the address history is: read once when it is first needed, and written
//! back once the program is done with it. A machine that does not name a local data directory
//! has nowhere to keep it, so a resource with no file is still a resource — it has simply
//! nowhere to write.

use std::fs;
use std::path::Path;

use mingling::{LazyInit, ProgramCollect, setup::ProgramSetup};

use crate::user::account_path;

/// The account the work acts as, as the program found it.
///
/// A resource is a value the program shares, so it is `Clone` and `Default`. The account is
/// the name alone, and whether it was changed is what decides whether the file is written
/// back — a run that only reads it leaves the file as it was.
#[derive(Debug, Default, Clone)]
pub struct ResCurrentAccount {
    /// The name the work acts as, if one has been named.
    name: Option<String>,
    /// Whether it was changed through [`set`](Self::set).
    changed: bool,
}

impl ResCurrentAccount {
    /// The name the work acts as, if one has been named.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Names the account the work acts as.
    ///
    /// What is named here is written back when the program is done with the resource.
    pub fn set(&mut self, name: impl Into<String>) {
        self.name = Some(name.into());
        self.changed = true;
    }

    /// Reads the name from `path`.
    ///
    /// What cannot be read is not an error: a file that is missing, or one that cannot be
    /// read at all, comes back as no account having been named.
    fn read(path: &Path) -> Self {
        let name = fs::read_to_string(path)
            .ok()
            .map(|text| text.trim().to_string())
            .filter(|name| !name.is_empty());

        Self {
            name,
            changed: false,
        }
    }

    /// Writes the name to `path`, if there is one to write.
    ///
    /// This runs as the resource is dropped, so a failure is silent — for the reason reading
    /// one is: there is nowhere left to report to.
    fn write(&self, path: &Path) {
        let Some(name) = &self.name else {
            return;
        };
        let Some(directory) = path.parent() else {
            return;
        };

        if fs::create_dir_all(directory).is_err() {
            return;
        }

        let _ = fs::write(path, format!("{name}\n"));
    }
}

/// Registers the account the work acts as, so commands can read it and name it.
///
/// This is Rorolala's own and is not part of the shared setups: which account the work acts
/// as is a choice of the user's, not something every program that links the setups keeps.
pub struct CurrentAccountSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for CurrentAccountSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        let path = account_path();
        let read_path = path.clone();
        let write_path = path;

        program.with_resource(
            ResCurrentAccount::lazy_init(move || {
                read_path
                    .as_deref()
                    .map_or_else(ResCurrentAccount::default, ResCurrentAccount::read)
            })
            .with_on_drop(move |account: ResCurrentAccount| {
                if account.changed
                    && let Some(path) = write_path.as_deref()
                {
                    account.write(path);
                }
            }),
        );
    }
}
