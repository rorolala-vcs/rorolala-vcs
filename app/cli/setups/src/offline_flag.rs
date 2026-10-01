use mingling::{
    ProgramCollect, Wrap,
    consts::REMAINS,
    macros::arg,
    picker::{IntoPicker, PickerArg, value::Flag},
    setup::ProgramSetup,
};

/// Represents whether the run may reach the Vault.
#[derive(Default, Clone, Wrap)]
pub struct ResOffline {
    offline: bool,
}

/// Reaches no Vault, and works from what is already here.
const ARG_OFFLINE: PickerArg<'static, Flag> = arg![offline: Flag];

/// A [`ProgramSetup`] implementation that registers whether the run is offline.
pub struct OfflineFlagSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for OfflineFlagSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        // UNWRAP: picking ARG_OFFLINE and REMAINS can each fall back, so parsing never
        // fails and the `unwrap` is safe.
        let (offline, args) = program
            .take_args()
            .pick(&ARG_OFFLINE)
            .pick(&REMAINS)
            .unwrap();
        program.replace_args(args.into());

        program.with_resource(ResOffline {
            offline: matches!(offline, Flag::Active),
        });
    }
}
