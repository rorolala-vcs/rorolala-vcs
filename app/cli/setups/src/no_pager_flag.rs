use std::io::IsTerminal as _;

use mingling::{
    ProgramCollect, Wrap,
    consts::REMAINS,
    macros::arg,
    picker::{IntoPicker, PickerArg, value::Flag},
    setup::ProgramSetup,
};

/// Represents whether a run may hand its output to a pager.
#[derive(Default, Clone, Wrap)]
pub struct ResNoPager {
    no_pager: bool,
}

/// Reaches no pager, and writes everything out where the run was made.
const ARG_NO_PAGER: PickerArg<'static, Flag> = arg![no_pager: Flag];

/// A [`ProgramSetup`] implementation that registers whether a run asked for no pager.
///
/// The answer is a resource rather than a flag a command declares, because what it settles is how
/// the whole run is written out, not what any one command does: the pager is registered by the
/// programs that have one, and reads this the way a command reads [`ResUsingVault`](super::ResUsingVault).
/// A command-level declaration would be dead code, since this pick eats the token first.
///
/// A run that is not talking to a terminal is answered as if it asked for no pager, whether or not
/// it did. The pager is for a person reading a screen, and a run whose output is a pipe or a file is
/// another program reading; a run whose input is not a terminal is one nobody is typing at, so a
/// pager waiting for a key would be waiting forever. That matters most for an agent driving the
/// command line: it reads the output through a pipe, and a pager there is a hang rather than a page.
pub struct NoPagerFlagSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for NoPagerFlagSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        // UNWRAP: picking ARG_NO_PAGER and REMAINS can each fall back, so parsing never
        // fails and the `unwrap` is safe.
        let (no_pager, args) = program
            .take_args()
            .pick(ARG_NO_PAGER)
            .pick(REMAINS)
            .unwrap();
        program.replace_args(args.into());

        let pageable = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();

        program.with_resource(ResNoPager {
            no_pager: matches!(no_pager, Flag::Active) || !pageable,
        });
    }
}
