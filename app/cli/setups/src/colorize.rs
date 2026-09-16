use mingling::{
    ProgramCollect,
    macros::arg,
    picker::{PickerArg, PickerHelper, value::Flag},
    setup::ProgramSetup,
};
use rorolala_utils_cli_theme::set_enabled;

/// Global flag that turns coloring off
///
/// Usage: `--no-color`
pub const GLOBAL_FLAG_NO_COLOR: PickerArg<'static, Flag> = arg![no_color: Flag];

/// A [`ProgramSetup`] implementation that turns coloring off when it is asked to
///
/// Absent the flag, nothing is decided, so the environment keeps its say — see
/// `rorolala_utils_cli_theme::is_enabled`. Asking for no color takes that decision over,
/// which is what an explicit request should do.
pub struct ColorizeSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for ColorizeSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        if matches!(
            program.pick_argument(&GLOBAL_FLAG_NO_COLOR),
            Some(Flag::Active)
        ) {
            set_enabled(false);
        }
    }
}
