//! `lib`: the C ABI artifact — a release shared and static library — and, as a side effect of the
//! root build script, the C header that describes it.

use std::ffi::OsStr;

use crate::paths::Workspace;
use crate::toolchain::Tool;
use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};

use crate::Next;
use crate::run_step;

/// Builds the library and the header that describes it.
#[command(node = "lib")]
pub fn library() -> StateLibrary {
    StateLibrary
}

/// The state the command starts in: it names nothing, since the library is what is built.
#[derive(Grouped)]
pub struct StateLibrary;

/// Builds the release library the C ABI is handed over as.
///
/// `--all-features` is what reaches the ABI the crate only carries behind a feature, and the header
/// is written by the root build script while the crate compiles, so building the artifact is also
/// what refreshes the header.
///
/// # Errors
///
/// Reports the build's own failure, and [`crate::Error::Directory`] when the workspace cannot name
/// where it builds.
pub fn build_library() -> Result<(), crate::Error> {
    let workspace = Workspace::discover()?;
    let arguments = ["build", "--release", "--all-features", "-p", "rorolala-ffi"];

    // A musl target links statically by default, and a shared library cannot be made against musl's
    // static C runtime: the linker refuses its relocations, since those objects are not position
    // independent. The shared library is the one artifact that needs the dynamic runtime, so the
    // flag is given to this build alone — the programs are built by their own step and keep the
    // static default, which is what makes them runnable on a machine with no musl installed.
    //
    // It is set on the child rather than asked for in the configuration because `RUSTFLAGS` in the
    // environment outranks the configuration: a caller that set it would silently get the static
    // runtime back, and a link error for its trouble.
    let musl = workspace
        .target_triple()
        .is_some_and(|triple| triple.contains("musl"));

    if musl {
        let flags = dynamic_c_runtime_flags(std::env::var("RUSTFLAGS").ok());

        return Tool::cargo().run_with_env(&[("RUSTFLAGS", OsStr::new(&flags))], arguments);
    }

    Tool::cargo().run(arguments)
}

/// The flags a musl shared library is built with: whatever the build already had, plus the one that
/// turns the static C runtime off.
fn dynamic_c_runtime_flags(existing: Option<String>) -> String {
    let mut flags = existing.unwrap_or_default().trim().to_owned();

    if !flags.is_empty() {
        flags.push(' ');
    }
    flags.push_str("-C target-feature=-crt-static");

    flags
}

/// Builds the library and the header.
#[chain]
pub fn handle_library(_state: StateLibrary, ec: &mut ResExitCode) -> Next {
    run_step(ec, build_library);

    empty_result!()
}

#[cfg(test)]
mod tests {
    use super::dynamic_c_runtime_flags;

    #[test]
    fn the_static_c_runtime_is_turned_off_without_losing_the_builds_own_flags() {
        assert_eq!(
            dynamic_c_runtime_flags(None),
            "-C target-feature=-crt-static"
        );

        // A build that came with flags keeps them: this adds one rather than replacing the set.
        assert_eq!(
            dynamic_c_runtime_flags(Some(String::from("-D warnings"))),
            "-D warnings -C target-feature=-crt-static"
        );

        // An empty or blank value asked for nothing, so it is not carried as a leading space.
        assert_eq!(
            dynamic_c_runtime_flags(Some(String::new())),
            "-C target-feature=-crt-static"
        );
        assert_eq!(
            dynamic_c_runtime_flags(Some(String::from("   "))),
            "-C target-feature=-crt-static"
        );
    }
}
