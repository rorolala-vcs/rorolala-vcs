#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use mingling::{ProgramCollect, setup::ProgramSetup};

///
pub struct RorolalaSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for RorolalaSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, _program: &mut mingling::Program<ThisProgram>) {
        todo!()
    }
}
