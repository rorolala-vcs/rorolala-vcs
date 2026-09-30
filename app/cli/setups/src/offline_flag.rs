use mingling::{
    ProgramCollect, Wrap,
    consts::REMAINS,
    macros::arg,
    picker::{IntoPicker, Pickable, value::Flag},
    setup::ProgramSetup,
};

/// Represents whether the run may reach the Vault.
#[derive(Default, Clone, Wrap)]
pub struct ResOffline {
    offline: bool,
}

/// The global flag that keeps a run from reaching the Vault.
#[derive(Pickable)]
pub struct OfflineFlags {
    /// Reaches no Vault, and works from what is already here.
    #[arg(long)]
    offline: Flag,
}

/// A [`ProgramSetup`] implementation that registers whether the run is offline.
pub struct OfflineFlagSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for OfflineFlagSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        // UNWRAP: `pick` for both `OfflineFlags` and `REMAINS` is infallible here — each pick can
        // fall back, so parsing never fails and the `unwrap` is safe.
        let (flags, args) = program
            .take_args()
            .pick(&arg![OfflineFlags])
            .pick(&REMAINS)
            .unwrap();
        program.replace_args(args.into());

        program.with_resource(ResOffline {
            offline: matches!(flags.offline, Flag::Active),
        });
    }
}
