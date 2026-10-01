use mingling::{
    ProgramCollect, Wrap,
    consts::REMAINS,
    macros::arg,
    picker::{IntoPicker, PickerArg, value::Flag},
    setup::ProgramSetup,
};

/// Represents whether the resource uses vault mode.
#[derive(Default, Clone, Wrap)]
pub struct ResUsingVault {
    using: bool,
}

/// Enables vault mode when set.
const ARG_VAULT: PickerArg<'static, Flag> = arg![vault: Flag, 'v'];

/// A [`ProgramSetup`] implementation that registers a global flag resource.
pub struct GlobalFlagSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for GlobalFlagSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        // UNWRAP: picking ARG_VAULT and REMAINS can each fall back, so parsing never
        // fails and the `unwrap` is safe.
        let (vault, args) = program.take_args().pick(&ARG_VAULT).pick(&REMAINS).unwrap();
        program.replace_args(args.into());

        program.with_resource(ResUsingVault {
            using: matches!(vault, Flag::Active),
        });
    }
}
