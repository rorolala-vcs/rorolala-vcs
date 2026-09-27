//! Whether a run that writes to the version control index also rebuilds the inverse index.
//!
//! The inverse index is derived data: it answers what the index cannot, and is left alone while it
//! still describes the index and ignored once it does not — a reader falls back to the objects. So
//! nothing depends on it being current, and rebuilding it after every write is a cost a run chooses
//! rather than one it pays: `--rebuild` is how a run says it wants the records kept in step.
//!
//! It is a resource rather than a plain flag so that a command reaches it the way it reaches the
//! rest of the run's shape — see [`ResUsingVault`](rorolala_cli_setups::ResUsingVault), which is
//! read from a global flag the same way.

use mingling::{
    ProgramCollect, Wrap,
    consts::REMAINS,
    macros::arg,
    picker::{IntoPicker as _, Pickable, value::Flag},
    setup::ProgramSetup,
};

/// Whether the inverse index is to be rebuilt after a write.
#[derive(Default, Clone, Wrap)]
pub struct ResRebuildInverseIndex {
    /// Whether `--rebuild` was given.
    rebuild: bool,
}

/// The global flags that say what a write does besides writing.
#[derive(Pickable)]
pub struct RebuildFlags {
    /// Rebuild the inverse index after writing.
    #[arg(long)]
    rebuild: Flag,
}

/// A [`ProgramSetup`] implementation that registers whether the inverse index is to be rebuilt.
pub struct RebuildSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for RebuildSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        // UNWRAP: each pick can fall back, so parsing never fails and the `unwrap` is safe — the
        // same reason `GlobalFlagSetup` gives for its own.
        let (flags, args) = program
            .take_args()
            .pick(&arg![RebuildFlags])
            .pick(&REMAINS)
            .unwrap();
        program.replace_args(args.into());

        program.with_resource(ResRebuildInverseIndex {
            rebuild: matches!(flags.rebuild, Flag::Active),
        });
    }
}
