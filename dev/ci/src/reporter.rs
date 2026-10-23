//! What a check leaves behind, so a run can be read as one page afterwards.
//!
//! The gate is a sequence of programs whose output goes to the terminal as it runs; what is left
//! here is the *shape* of that run — which checks passed, which failed, and where — in the form
//! `report-collect` renders. A step that cannot be recorded does not stop the run: the report is a
//! reading of a run, never a precondition for one.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use rola_build::Error;
use rola_build::paths::Workspace;

/// Where the reports live, inside cargo's cache: build output, like everything else there.
const REPORTS: &str = ".cache/reports";

/// The directory a run's outcomes are collected in, one file per check and item.
const COLLECT: &str = "collect";

/// The file `report-collect` renders the run into.
const RESULT: &str = "result.md";

/// Where a run's outcomes are left.
pub struct Reporter {
    root: Option<PathBuf>,
}

impl Reporter {
    /// A reporter over this workspace, or one that records nothing when there is no workspace to
    /// record it in.
    #[must_use]
    pub fn new() -> Self {
        let root = Workspace::discover()
            .ok()
            .map(|workspace| workspace.root().join(REPORTS));

        if root.is_none() {
            eprintln!("warning: not inside a workspace; this run is not recorded");
        }

        Self { root }
    }

    /// Removes what a previous run left, so a report is of the run that follows it.
    pub fn clean(&self) {
        let Some(root) = &self.root else {
            return;
        };

        if let Err(error) = std::fs::remove_dir_all(root.join(COLLECT))
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("warning: the collected report could not be removed: {error}");
        }

        if let Err(error) = std::fs::remove_file(root.join(RESULT))
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("warning: the rendered report could not be removed: {error}");
        }
    }

    /// The log for one check, which its outcomes are written to as it runs.
    #[must_use]
    pub fn task(&self, task: &str) -> TaskLog {
        TaskLog {
            directory: self.root.as_ref().map(|root| root.join(COLLECT)),
            stem: format!("{task}.{}", std::env::consts::OS),
        }
    }

    /// Renders what the run collected into `result.md`, and answers where it was written.
    ///
    /// # Errors
    ///
    /// Reports [`Error::Io`] when the report cannot be written.
    pub fn collect(&self) -> Result<Option<PathBuf>, Error> {
        let Some(root) = &self.root else {
            return Ok(None);
        };

        let outcomes = read_collected(&root.join(COLLECT));
        let report = render(&outcomes, std::env::consts::OS);
        let result = root.join(RESULT);

        if let Some(directory) = result.parent() {
            std::fs::create_dir_all(directory).map_err(|error| Error::Io {
                doing: "creating the reports directory",
                detail: format!("{}: {error}", directory.display()),
            })?;
        }

        std::fs::write(&result, report).map_err(|error| Error::Io {
            doing: "writing the report",
            detail: format!("{}: {error}", result.display()),
        })?;

        Ok(Some(result))
    }
}

/// One check's log: what it ran, and how each of its items ended.
pub struct TaskLog {
    directory: Option<PathBuf>,
    stem: String,
}

impl TaskLog {
    /// Records that `item` passed, naming where it is.
    pub fn passed(&self, item: &str, location: &str) {
        let Some(directory) = &self.directory else {
            return;
        };

        if let Err(error) = write_ok(directory, &self.stem, item, location) {
            eprintln!("warning: {item} passed and that was not recorded: {error}");
        }
    }

    /// Records that `item` failed, naming where it is and what it said.
    pub fn failed(&self, item: &str, location: &str, detail: &str) {
        let Some(directory) = &self.directory else {
            return;
        };

        if let Err(error) = write_err(directory, &self.stem, item, location, detail) {
            eprintln!("warning: {item} failed and that was not recorded: {error}");
        }
    }
}

/// One recorded outcome of a run.
struct Outcome {
    task: String,
    item: String,
    location: String,
    detail: String,
    passed: bool,
}

/// Appends a pass to the check's `.ok` file.
fn write_ok(directory: &Path, stem: &str, item: &str, location: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(directory)?;

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join(format!("{stem}.ok")))?;
    file.write_all(format!("{item} = {location}\n").as_bytes())?;

    Ok(())
}

/// Writes a failure to its own file, whose first line is where it was and whose rest is what it
/// said.
fn write_err(
    directory: &Path,
    stem: &str,
    item: &str,
    location: &str,
    detail: &str,
) -> std::io::Result<()> {
    std::fs::create_dir_all(directory)?;

    let body = format!("{location}\n{detail}\n");

    std::fs::write(
        directory.join(format!(
            "{stem}.{}.err",
            item.replace(['/', '\\', ' '], "-")
        )),
        body,
    )
}

/// What a run collected, read back for rendering.
fn read_collected(directory: &Path) -> Vec<Outcome> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            eprintln!(
                "warning: the collected report could not be read: {}: {error}",
                directory.display()
            );
            return Vec::new();
        }
    };

    let mut outcomes = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
            continue;
        };
        let Some(text) = std::fs::read_to_string(&path).ok() else {
            continue;
        };

        if let Some(rest) = name.strip_suffix(".ok") {
            for line in text.lines().filter(|line| !line.trim().is_empty()) {
                let (item, location) = line.split_once(" = ").unwrap_or((line, ""));
                outcomes.push(Outcome {
                    task: task_of(rest),
                    item: item.to_owned(),
                    location: location.to_owned(),
                    detail: String::new(),
                    passed: true,
                });
            }
        } else if let Some(rest) = name.strip_suffix(".err") {
            let mut lines = text.lines();
            let location = lines.next().unwrap_or_default();
            let detail = lines.collect::<Vec<&str>>().join("\n");
            let (stem, item) = rest.rsplit_once('.').unwrap_or((rest, ""));
            outcomes.push(Outcome {
                task: task_of(stem),
                item: item.to_owned(),
                location: location.to_owned(),
                detail: detail.trim_end().to_owned(),
                passed: false,
            });
        }
    }

    outcomes.sort_by(|left, right| {
        left.task
            .cmp(&right.task)
            .then_with(|| left.item.cmp(&right.item))
    });
    outcomes
}

/// The check's name, from a file name whose last part is the platform it ran on.
fn task_of(stem: &str) -> String {
    stem.rsplit_once('.')
        .map_or(stem, |(task, _)| task)
        .to_owned()
}

/// The run, as one page.
fn render(outcomes: &[Outcome], platform: &str) -> String {
    let mut report = String::new();

    report.push_str("# rola CI report\n\n");
    let _ = writeln!(
        report,
        "{} outcome(s) recorded on {platform}.\n",
        outcomes.len()
    );

    if outcomes.is_empty() {
        report.push_str(
            "Nothing was recorded: no check was run, or the run could not write its outcomes.\n",
        );
        return report;
    }

    report.push_str("| check | item | result |\n| --- | --- | --- |\n");
    for outcome in outcomes {
        let result = if outcome.passed { "pass" } else { "FAIL" };
        let _ = writeln!(report, "| {} | {} | {result} |", outcome.task, outcome.item);
    }

    let failures: Vec<&Outcome> = outcomes.iter().filter(|outcome| !outcome.passed).collect();
    if !failures.is_empty() {
        report.push_str("\n## Failures\n");
        for outcome in failures {
            let _ = writeln!(report, "\n### {} / {}\n", outcome.task, outcome.item);
            let _ = writeln!(report, "`{}`\n", outcome.location);
            if !outcome.detail.is_empty() {
                report.push_str("```\n");
                report.push_str(&outcome.detail);
                report.push_str("\n```\n");
            }
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::{Outcome, render, task_of};

    #[test]
    fn a_checks_name_is_the_stem_without_the_platform() {
        assert_eq!(task_of("fmt-check.linux"), "fmt-check");
        assert_eq!(task_of("check.windows"), "check");
        assert_eq!(task_of("fmt-check"), "fmt-check");
    }

    #[test]
    fn a_run_is_rendered_as_a_table_and_its_failures_below() {
        let outcomes = vec![
            Outcome {
                task: "fmt-check".to_owned(),
                item: "workspace".to_owned(),
                location: "cargo fmt --all -- --check".to_owned(),
                detail: String::new(),
                passed: true,
            },
            Outcome {
                task: "test".to_owned(),
                item: "workspace".to_owned(),
                location: "cargo test --workspace".to_owned(),
                detail: "a test failed".to_owned(),
                passed: false,
            },
        ];

        let report = render(&outcomes, "windows");
        assert!(report.contains("2 outcome(s) recorded on windows"));
        assert!(report.contains("| fmt-check | workspace | pass |"));
        assert!(report.contains("| test | workspace | FAIL |"));
        assert!(report.contains("### test / workspace"));
        assert!(report.contains("a test failed"));
    }

    #[test]
    fn a_run_nothing_was_recorded_of_says_so_rather_than_rendering_an_empty_table() {
        let report = render(&[], "linux");
        assert!(report.contains("Nothing was recorded"));
        assert!(!report.contains("| check |"));
    }
}
