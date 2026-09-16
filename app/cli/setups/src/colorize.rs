use mingling::{
    ProgramCollect,
    macros::arg,
    picker::{PickerArg, PickerHelper, value::Flag},
    setup::ProgramSetup,
};
use rorolala_utils_cli_theme::{ThemeChoice, set_enabled, set_theme_choice};

/// Global flag that turns coloring off
///
/// Usage: `--no-color`
pub const GLOBAL_FLAG_NO_COLOR: PickerArg<'static, Flag> = arg![no_color: Flag];

/// Global argument that names the theme to draw in
///
/// Usage: `--theme-choice=pretty`
pub const GLOBAL_ARG_THEME_CHOICE: PickerArg<'static, ThemeChoice> =
    arg![theme_choice: ThemeChoice];

/// A [`ProgramSetup`] implementation that decides how output is drawn
///
/// Both the flags it reads are the same kind of request — how the program is to appear
/// — and both are about the same small decision, so they are registered together.
///
/// Absent either flag, nothing is decided: the environment keeps its say over coloring
/// and the terminal over the theme. Asking takes that decision over, which is what an
/// explicit request should do.
pub struct ColorizeSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for ColorizeSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        if let Some(choice) = program.pick_argument(&GLOBAL_ARG_THEME_CHOICE) {
            set_theme_choice(choice);
        }

        if matches!(
            program.pick_argument(&GLOBAL_FLAG_NO_COLOR),
            Some(Flag::Active)
        ) {
            set_enabled(false);
        }
    }
}
