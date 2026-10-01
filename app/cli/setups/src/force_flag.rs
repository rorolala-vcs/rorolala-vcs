use mingling::{
    ProgramCollect, Wrap,
    consts::REMAINS,
    macros::arg,
    picker::{IntoPicker, PickerArg, value::Flag},
    setup::ProgramSetup,
};

/// Represents whether the run goes on where a command would otherwise refuse.
#[derive(Default, Clone, Wrap)]
pub struct ResForce {
    force: bool,
}

/// Goes on anyway where the command would refuse.
const ARG_FORCE: PickerArg<'static, Flag> = arg![force: Flag];

/// A [`ProgramSetup`] implementation that registers whether the run is forced.
pub struct ForceFlagSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for ForceFlagSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        // UNWRAP: picking ARG_FORCE and REMAINS can each fall back, so parsing never
        // fails and the `unwrap` is safe.
        let (force, args) = program.take_args().pick(&ARG_FORCE).pick(&REMAINS).unwrap();
        program.replace_args(args.into());

        program.with_resource(ResForce {
            force: matches!(force, Flag::Active),
        });
    }
}
