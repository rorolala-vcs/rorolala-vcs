use mingling::{
    ProgramCollect,
    setup::{
        DirectoryEnvironmentSetup, ExitCodeSetup, ProgramSetup,
        picker::{ConfirmFlagSetup, HelpFlagSetup, QuietFlagSetup, StructuralRendererSetup},
    },
};

/// The framework's `DefaultSetup`, with its version and verbose flags left out.
///
/// `DefaultSetup` mounts `BasicProgramSetup`, which claims `-v`/`--version` and `-V`/`--verbose`.
/// Rorolala's programs spell those otherwise: `-v` is the short name of `--vault` (see
/// [`GlobalFlagSetup`](crate::GlobalFlagSetup)), and `-V`/`--version` is answered by the program's
/// own version node rather than by the framework's version query. The picker takes a spelling it
/// claims before any command sees it, so the two flags are not mounted at all; what is registered
/// here is everything else `DefaultSetup` registers — help, quiet, confirm, the structural renderer,
/// the exit code and the directory environment — and nothing more.
pub struct RorolalaDefaultSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for RorolalaDefaultSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram> + 'static,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        program.with_setup(HelpFlagSetup::default());
        program.with_setup(QuietFlagSetup::default());
        program.with_setup(ConfirmFlagSetup::default());
        program.with_setup(StructuralRendererSetup);
        program.with_setup(ExitCodeSetup);
        program.with_setup(DirectoryEnvironmentSetup);
    }
}
