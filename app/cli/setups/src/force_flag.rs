use mingling::{
    ProgramCollect, Wrap,
    consts::REMAINS,
    macros::arg,
    picker::{IntoPicker, Pickable, value::Flag},
    setup::ProgramSetup,
};

/// Represents whether the run goes on where a command would otherwise refuse.
#[derive(Default, Clone, Wrap)]
pub struct ResForce {
    force: bool,
}

/// The global flag that lets a run go on anyway.
#[derive(Pickable)]
pub struct ForceFlags {
    /// Goes on anyway where the command would refuse.
    #[arg(long)]
    force: Flag,
}

/// A [`ProgramSetup`] implementation that registers whether the run is forced.
pub struct ForceFlagSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for ForceFlagSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        // UNWRAP: `pick` for both `ForceFlags` and `REMAINS` is infallible here — each pick can
        // fall back, so parsing never fails and the `unwrap` is safe.
        let (flags, args) = program
            .take_args()
            .pick(&arg![ForceFlags])
            .pick(&REMAINS)
            .unwrap();
        program.replace_args(args.into());

        program.with_resource(ResForce {
            force: matches!(flags.force, Flag::Active),
        });
    }
}
