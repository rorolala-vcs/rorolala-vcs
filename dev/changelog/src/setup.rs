//! `--allow-dirty`, picked out of the arguments before a command runs.
//!
//! The flag is global on purpose: the three commands that write the tree all meet the same refusal,
//! and declaring it on each of them would be three names for one answer. It is consumed here, so a
//! command that asks for the resource reads the answer rather than parsing the word.

use mingling::{
    ProgramCollect, Wrap,
    consts::REMAINS,
    macros::arg,
    picker::{IntoPicker, PickerArg, value::Flag},
    setup::ProgramSetup,
};

/// Represents whether a command may write a tree that has changes in it.
#[derive(Default, Clone, Wrap)]
pub struct ResAllowDirty {
    allow: bool,
}

impl ResAllowDirty {
    /// Whether the run asked to go on over a dirty tree.
    #[must_use]
    pub const fn allowed(&self) -> bool {
        self.allow
    }
}

/// Goes on where a command would otherwise refuse a dirty tree.
const ARG_ALLOW_DIRTY: PickerArg<'static, Flag> = arg![allow_dirty: Flag];

/// A [`ProgramSetup`] implementation that registers whether a dirty tree is allowed.
pub struct AllowDirtySetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for AllowDirtySetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        // UNWRAP: picking the flag and the words left over can each fall back, so neither fails.
        let (allow, args) = program
            .take_args()
            .pick(ARG_ALLOW_DIRTY)
            .pick(REMAINS)
            .unwrap();
        program.replace_args(args.into());

        program.with_resource(ResAllowDirty {
            allow: matches!(allow, Flag::Active),
        });
    }
}
