//! `coverage`: the tests and the integration suites under LLVM source-based coverage.
//!
//! The environment is `cargo llvm-cov show-env`, which names the instrumenting `RUSTC_WRAPPER` and
//! the profraw template. The two phases are the gate's own commands, run as children, so nothing
//! here restates what a test run or a suite run is. The report counts `src/` and `modules/` only.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use mingling::{
    Grouped,
    macros::{chain, command, empty_result},
    res::ResExitCode,
};
use rola_build::{Error, paths::Workspace, toolchain::Tool};

use super::run_check;
use crate::Next;
use crate::reporter::Reporter;

/// Where the report is written, under the workspace root.
const COVERAGE: &str = ".cache/reports/coverage";

/// The files that are instrumented but are not what this report measures.
///
/// `cargo-llvm-cov` can only ignore, so the pattern names every directory the workspace holds
/// besides `src/` and `modules/`. `build.rs` and `protocol-macros` run at compile time, and a line
/// about them is not a line about behaviour.
const IGNORE: &str = r"(^|[/\\])(app|dev|tests|utils)([/\\])|(^|[/\\])protocol-macros([/\\])|(^|[/\\])(Cargo|\.cargo)([/\\])registry([/\\])|(^|[/\\])rustc([/\\])|(^|[/\\])build\.rs$";

/// Runs the tests and the integration suites under LLVM coverage.
#[command(node = "coverage")]
pub fn coverage() -> StateCoverage {
    StateCoverage
}

/// The state the coverage run starts in: it names nothing, since every phase is what is run.
#[derive(Grouped)]
pub struct StateCoverage;

/// Runs both phases, then writes the report, and reports a failure of either phase.
#[chain]
pub fn handle_coverage(_state: StateCoverage, ec: &mut ResExitCode) -> Next {
    Reporter::new().clean();

    run_check("coverage", "cargo ci coverage", ec, run_coverage);

    if let Err(error) = Reporter::new().collect() {
        eprintln!("warning: {error}");
    }

    empty_result!()
}

/// Runs the phases under the coverage environment, then writes the report.
///
/// # Errors
///
/// Reports whatever `cargo llvm-cov` or a phase reports, and [`Error::Io`] when the report
/// directory cannot be made.
fn run_coverage() -> Result<(), Error> {
    let workspace = Workspace::discover()?;

    // A report is of this run: profraw files left by another one are not part of it.
    Tool::cargo().run(["llvm-cov", "clean", "--profraw-only"])?;

    let environment = show_env()?;
    let environment: Vec<(&str, &OsStr)> = environment
        .iter()
        .map(|(name, value)| (name.as_str(), OsStr::new(value.as_str())))
        .collect();

    let test_failed = Tool::cargo()
        .run_with_env(&environment, ["ci", "test"])
        .inspect_err(|error| eprintln!("{error}"))
        .is_err();

    let integration_failed = Tool::cargo()
        .run_with_env(&environment, ["ci", "integration-test"])
        .inspect_err(|error| eprintln!("{error}"))
        .is_err();

    write_report(&workspace, &environment)?;

    if test_failed || integration_failed {
        return Err(Error::Status {
            program: "the coverage run".to_owned(),
            code: 1,
        });
    }

    Ok(())
}

/// The environment `cargo llvm-cov` builds and runs under.
///
/// # Errors
///
/// Reports [`Error::Tool`] when `cargo` cannot be started, and [`Error::Missing`] when
/// `cargo llvm-cov show-env` did not answer.
fn show_env() -> Result<Vec<(String, String)>, Error> {
    let output = Tool::cargo().capture(["llvm-cov", "show-env"])?;

    if !output.status.success() {
        return Err(Error::Missing {
            summary: "`cargo llvm-cov` could not read its environment".to_owned(),
            advice: vec![
                "cargo install cargo-llvm-cov --version 0.8.7 --locked".to_owned(),
                "rustup component add llvm-tools-preview".to_owned(),
            ],
        });
    }

    Ok(parse_show_env(&String::from_utf8_lossy(&output.stdout)))
}

/// Reads `NAME='value'` lines into pairs.
fn parse_show_env(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let (name, value) = line.split_once('=')?;
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            let value = value.trim();
            let value = value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
                .unwrap_or(value);
            Some((name.to_owned(), value.to_owned()))
        })
        .collect()
}

/// Writes the summary, the lcov file and the html pages.
///
/// # Errors
///
/// Reports [`Error::Io`] when the directory cannot be made, and whatever `cargo llvm-cov report`
/// reports after that.
fn write_report(workspace: &Workspace, environment: &[(&str, &OsStr)]) -> Result<(), Error> {
    let directory = workspace.root().join(COVERAGE);
    std::fs::create_dir_all(&directory).map_err(|error| Error::Io {
        doing: "creating the coverage report directory",
        detail: format!("{}: {error}", directory.display()),
    })?;

    let manifest = workspace.manifest_path();
    let lcov = directory.join("lcov.info");
    let html = directory.join("html");

    report(&manifest, environment, &[])?;
    report(
        &manifest,
        environment,
        &[
            OsStr::new("--lcov"),
            OsStr::new("--output-path"),
            lcov.as_os_str(),
        ],
    )?;
    report(
        &manifest,
        environment,
        &[
            OsStr::new("--html"),
            OsStr::new("--output-dir"),
            html.as_os_str(),
        ],
    )
}

/// Runs `cargo llvm-cov report` over the manifest, with the report environment and `extra`.
fn report(manifest: &Path, environment: &[(&str, &OsStr)], extra: &[&OsStr]) -> Result<(), Error> {
    let mut arguments: Vec<OsString> = vec![
        OsString::from("llvm-cov"),
        OsString::from("report"),
        OsString::from("--workspace"),
        OsString::from("--manifest-path"),
        manifest.as_os_str().to_owned(),
        OsString::from("--ignore-filename-regex"),
        OsString::from(IGNORE),
    ];
    arguments.extend(extra.iter().map(|argument| (*argument).to_owned()));

    Tool::cargo().run_with_env(environment, arguments)
}

#[cfg(test)]
mod tests {
    use super::parse_show_env;

    #[test]
    fn the_environment_is_read_as_names_and_values() {
        let parsed =
            parse_show_env("LLVM_PROFILE_FILE='a b-%p.profraw'\nRUSTC_WRAPPER=cargo-llvm-cov\n\n");

        assert_eq!(
            parsed,
            vec![
                ("LLVM_PROFILE_FILE".to_owned(), "a b-%p.profraw".to_owned()),
                ("RUSTC_WRAPPER".to_owned(), "cargo-llvm-cov".to_owned()),
            ]
        );
    }
}
