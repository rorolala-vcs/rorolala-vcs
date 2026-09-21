//! The exit code of the last run that failed, kept in Rorolala's own file for the user.
//!
//! It is what `rola explain exit-code` explains when it is given no number: the run just
//! before is the one being asked about, and all of it that is kept is a number. Only a run
//! that failed is written down — a run that did nothing wrong has nothing to explain, and
//! writing it would bury the failure a reader still has a question about.
//!
//! It is the smallest of the files Rorolala keeps — written once as a run ends, read once
//! when it is asked about — and a machine that does not name a local data directory has
//! nowhere to keep it, so both sides stay silent about a file they cannot reach.

use std::fs;

use mingling::{
    ProgramCollect,
    hook::{ProgramControlUnit, ProgramControls, ProgramHook},
    res::ResExitCode,
    setup::ProgramSetup,
};

use crate::user::lastec_path;

/// Records `code` as the exit code of this run.
///
/// This is called only for a run that failed, so the file holds the last failure rather
/// than the last run. It runs as the run ends, so a failure here is silent: there is nothing
/// left to report it to, and a record that was not kept only means the next `rola explain
/// exit-code` has nothing to explain.
pub fn record(code: i32) {
    let Some(path) = lastec_path() else {
        return;
    };
    let Some(directory) = path.parent() else {
        return;
    };

    if fs::create_dir_all(directory).is_err() {
        return;
    }

    // A trailing newline makes the file read as one line, which is what `last` expects of
    // it and what anything that opens it in an editor shows as a line rather than a stray
    // number at the start of one.
    let _ = fs::write(path, format!("{code}\n"));
}

/// The exit code the last run that failed ended with, if one was recorded.
///
/// What cannot be read is not told apart from what was never written: a file that is
/// missing and one that does not hold a number both come back as no code, which is the one
/// answer a caller can act on.
#[must_use]
pub fn last() -> Option<i32> {
    let text = fs::read_to_string(lastec_path()?).ok()?;

    text.trim().parse().ok()
}

/// Registers the hook that keeps what a run left behind.
///
/// What a run ended with is not the caller's to read and write from outside: a run ends
/// through the pipeline, and the one point every path through it passes is the `finish` hook.
/// Two things are done with the code there, and nowhere else: a run that failed is written
/// down for `rola explain exit-code` to speak about, and a completion run is made to end with
/// nothing wrong whatever it did, since a shell asking what could be typed has not run
/// anything to go wrong.
///
/// The hook answers with nothing where it has nothing to say, which is what lets it be
/// registered after the exit-code setup: hooks are gathered in the order they were registered,
/// so saying nothing leaves what that one said standing.
///
/// Doing the work here rather than after the program is run is what keeps the program in the
/// pipeline: running it is also what tears it down, and tearing it down is what runs every
/// resource's own write-back — so a run that ended has everything it kept already written by
/// the time there is a process exit code to look at.
pub struct LastExitCodeRecordSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for LastExitCodeRecordSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram> + 'static,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        // Whether this run is the shell asking what could be typed rather than a run of the
        // program: it is read here, where the arguments are still at hand, and carried into
        // the hook, which is handed only the finished result.
        let completing = program.is_completing();

        program.with_hook(ProgramHook::empty().on_finish(move |_| {
            if completing {
                return ProgramControls::Single(ProgramControlUnit::OverrideExitCode(0));
            }

            let exit_code = exit_code::<ThisProgram>();
            if exit_code != 0 {
                record(exit_code);
            }

            ProgramControls::Empty
        }));
    }
}

/// The exit code the run ended with, as the program holds it.
fn exit_code<ThisProgram>() -> i32
where
    ThisProgram: ProgramCollect<Enum = ThisProgram> + 'static,
{
    // The code is a resource of the program, and the program is reachable while it runs: this
    // reads it the way the framework's own `update_exit_code` writes it, so the two cannot
    // come to look at different places.
    let mut held_code = 0;
    mingling::this::<ThisProgram>().modify_res(|held: &mut ResExitCode| held_code = held.exit_code);

    held_code
}
