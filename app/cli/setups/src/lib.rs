#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use mingling::{ProgramCollect, setup::ProgramSetup};

mod colorize;
mod global_flag;
mod language;
mod progress;
mod storage;
mod vault;
mod workspace;

pub use colorize::*;
pub use global_flag::*;
pub use language::*;
pub use progress::*;
pub use storage::*;
pub use vault::*;
pub use workspace::*;

/// Shared setup for Rorolala's command-line programs.
///
/// Registers the resources, global flags and hooks common to every program.
/// Commands are deliberately not registered here: `gen_program!()` collects
/// commands per crate, so a command must be declared in the crate that binds it.
///
/// The setups that shape the output — the language it is written in, how it is drawn and how
/// it reports what it is doing — come first, so that everything registered after them is
/// spoken, rendered and reported the way the run asked for.
pub struct RorolalaSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for RorolalaSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        program.with_setup(LanguageSetup);
        program.with_setup(ColorizeSetup);
        program.with_setup(ProgressSetup);
        program.with_setup(GlobalFlagSetup);
        program.with_setup(VaultSetup);
        program.with_setup(WorkspaceSetup);
        program.with_setup(RorolalaStorageSetup);
    }
}
