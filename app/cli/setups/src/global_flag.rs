use mingling::{
    ProgramCollect, Wrap,
    consts::REMAINS,
    macros::arg,
    picker::{IntoPicker, Pickable, value::Flag},
    setup::ProgramSetup,
};

/// Represents whether the resource uses vault mode.
#[derive(Default, Clone, Wrap)]
pub struct ResUsingVault {
    using: bool,
}

/// Represents the global command-line flags.
#[derive(Pickable)]
pub struct GlobalFlags {
    /// Enables vault mode when set.
    #[arg(short, long)]
    vault: Flag,
}

/// A [`ProgramSetup`] implementation that registers a global flag resource.
pub struct GlobalFlagSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for GlobalFlagSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        // UNWRAP: `pick` for both `GlobalFlags` and `REMAINS` is infallible here — each pick can
        // fall back, so parsing never fails and the `unwrap` is safe.
        let (global_flags, args) = program
            .take_args()
            .pick(&arg![GlobalFlags])
            .pick(&REMAINS)
            .unwrap();
        program.replace_args(args.into());

        program.with_resource(ResUsingVault {
            using: matches!(global_flags.vault, Flag::Active),
        });
    }
}
