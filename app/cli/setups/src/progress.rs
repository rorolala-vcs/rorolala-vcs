use mingling::{
    ProgramCollect,
    config::StructuralRendererSetting,
    macros::arg,
    picker::{PickerArg, PickerHelper, value::Flag},
    setup::ProgramSetup,
};

/// Global flag that turns progress reporting off
///
/// Usage: `--no-progress`
pub const GLOBAL_FLAG_NO_PROGRESS: PickerArg<'static, Flag> = arg![no_progress: Flag];

/// How a run is to say what it is doing while it does it.
///
/// A run says its progress down a channel and leaves what becomes of it to the reader, so
/// this is the reader's whole choice: a terminal redrawn with bars, a stream of records for
/// another program to read, or nothing at all.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ResProgressSetting {
    /// A terminal is redrawn with a bar and a spinner
    ///
    /// This is what a run that asked for nothing gets: a person watching a run is the common
    /// case, and it is the one that should need no saying.
    #[default]
    Indicatif,
    /// One record per change, for another program to read
    Jsonl,
    /// Nothing is said
    No,
}

/// A [`ProgramSetup`] implementation that decides how a run reports its progress
///
/// The choice is made of two requests a run can already make, so it needs no flag of its
/// own: `--no-progress` says nothing is to be shown, and a structural renderer — `--json`
/// and its kind — says the run is being read by a program rather than watched by a person,
/// which is what the records are for. Asking for neither is a person watching, and that is
/// what a terminal is redrawn for.
///
/// The structural renderer is read from where the setup before this one put it rather than
/// from the flag: `--json` is the renderer's flag, and by the time this runs it has been taken.
pub struct ProgressSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for ProgressSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        let asked_for_nothing = matches!(
            program.pick_argument(&GLOBAL_FLAG_NO_PROGRESS),
            Some(Flag::Active)
        );

        let read_by_a_program = matches!(
            program.structural_renderer_name,
            StructuralRendererSetting::Json | StructuralRendererSetting::JsonPretty
        );

        let setting = if asked_for_nothing {
            ResProgressSetting::No
        } else if read_by_a_program {
            ResProgressSetting::Jsonl
        } else {
            ResProgressSetting::Indicatif
        };

        program.with_resource(setting);
    }
}
