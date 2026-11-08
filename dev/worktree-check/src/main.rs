//! Reports the state of the worktree: fetch age, `HEAD`, local changes and remotes.
//!
//! It is reached as `cargo worktree-check`, and it reads the repository the current directory is
//! in. Nothing here decides what a branch should be: the tool reports, and only `--fetch` writes,
//! recording when the last successful fetch happened so the next run can say how stale the view is.

#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
#![deny(missing_docs)]

use std::env;
use std::fmt;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The official repository in the normalized form [`normalize_remote`] produces.
const OFFICIAL_REPOSITORY: &str = "github.com/rorolala-vcs/rorolala-vcs";

/// Where the last successful fetch is recorded, relative to the repository root.
const FETCH_STAMP: &str = "dev/worktree-check/__last_fetch";

const USAGE: &str = "\
usage:
  cargo worktree-check          report fetch age, HEAD, changes and remotes
  cargo worktree-check --fetch  fetch every remote, record the time, then report
  cargo worktree-check --help   print this usage";

/// Writes one line to standard output.
///
/// A closed pipe is not a failure: `cargo worktree-check | head` closes stdout early, and the
/// report is not worth a panic after the reader has stopped reading.
fn say(args: fmt::Arguments<'_>) {
    let mut stdout = io::stdout().lock();
    let _ = writeln!(stdout, "{args}");
}

macro_rules! say {
    () => {
        say(format_args!(""))
    };
    ($($arg:tt)*) => {
        say(format_args!($($arg)*))
    };
}

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Usage) => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
        Err(Failure::Message(message)) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
        // Git wrote its own failure to the inherited stderr, so repeating it would say it twice.
        Err(Failure::Quiet) => ExitCode::from(1),
    }
}

/// What ended a run. Git failures carry the text to print, because a caller that only sees an exit
/// status cannot tell a missing repository from a failed fetch.
enum Failure {
    /// An argument the tool does not take.
    Usage,
    /// A message the tool writes itself.
    Message(String),
    /// A failure another program has already reported on stderr.
    Quiet,
}

fn run(args: impl Iterator<Item = String>) -> Result<(), Failure> {
    let mut fetch = false;
    let mut help = false;
    for arg in args {
        match arg.as_str() {
            "--fetch" => fetch = true,
            "--help" => help = true,
            _ => return Err(Failure::Usage),
        }
    }
    if help {
        say!("{USAGE}");
        return Ok(());
    }

    let root = repository_root()?;
    if fetch {
        fetch_remotes(&root)?;
        record_fetch(&root)?;
    }
    report(&root)
}

/// Locates the repository from the working directory rather than assuming the program was started
/// at its root, so a run from a subdirectory reports the same tree as a run from the root.
fn repository_root() -> Result<PathBuf, Failure> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|err| Failure::Message(format!("cannot run git: {err}")))?;
    if !output.status.success() {
        return Err(Failure::Message(stderr_text(&output.stderr)));
    }
    let root = String::from_utf8_lossy(&output.stdout);
    let root = root.trim();
    if root.is_empty() {
        return Err(Failure::Message(
            "git did not name a repository root".to_owned(),
        ));
    }
    Ok(PathBuf::from(root))
}

/// Runs one git command in the repository and returns its stdout.
///
/// A failure becomes the command's stderr, so the caller prints what git said rather than a
/// paraphrase of it.
fn git(root: &Path, args: &[&str]) -> Result<String, Failure> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|err| Failure::Message(format!("cannot run git {}: {err}", args.join(" "))))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(Failure::Message(stderr_text(&output.stderr)))
    }
}

fn stderr_text(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let text = text.trim_end();
    if text.is_empty() {
        "git failed without a message".to_owned()
    } else {
        text.to_owned()
    }
}

/// The fetch keeps git's own streams, because its progress and its failure are what a person
/// watching a fetch expects to see while it runs.
fn fetch_remotes(root: &Path) -> Result<(), Failure> {
    let status = Command::new("git")
        .args(["fetch", "--all", "--prune"])
        .current_dir(root)
        .status()
        .map_err(|err| Failure::Message(format!("cannot run git fetch: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(Failure::Quiet)
    }
}

fn record_fetch(root: &Path) -> Result<(), Failure> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| Failure::Message(format!("the clock is before the Unix epoch: {err}")))?;
    let path = root.join(FETCH_STAMP);
    std::fs::write(&path, format!("{}\n", now.as_secs()))
        .map_err(|err| Failure::Message(format!("cannot write {}: {err}", path.display())))
}

fn read_fetch(root: &Path) -> Option<SystemTime> {
    let text = std::fs::read_to_string(root.join(FETCH_STAMP)).ok()?;
    let seconds = text.trim().parse::<u64>().ok()?;
    UNIX_EPOCH.checked_add(Duration::from_secs(seconds))
}

fn report(root: &Path) -> Result<(), Failure> {
    say!("worktree-check");
    say!();
    match read_fetch(root) {
        Some(recorded) => say!("last fetch: {}", describe_fetch(recorded)),
        None => say!("last fetch: never recorded"),
    }

    let head = head(root)?;
    say!("head: {}", head.branch);
    say!("ref: {}", head.refname);
    say!("commit: {}", short(&head.commit));
    say!("detached: {}", yes_no(head.detached));
    say!();

    let changes = changes(root)?;
    print_group("staged changes", &changes.staged);
    print_group("unstaged changes", &changes.unstaged);
    print_group("untracked paths", &changes.untracked);
    say!();

    report_remotes(&remotes(root)?);
    Ok(())
}

fn describe_fetch(recorded: SystemTime) -> String {
    SystemTime::now()
        .duration_since(recorded)
        .map_or_else(|_| "in the future".to_owned(), format_age)
}

/// Renders a span as the largest whole unit it fills, so a reader gets one number to compare rather
/// than a count in seconds to divide in his head.
fn format_age(age: Duration) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    const MONTH: u64 = 30 * DAY;
    const YEAR: u64 = 365 * DAY;

    let seconds = age.as_secs();
    if seconds < MINUTE {
        "just now".to_owned()
    } else if seconds < HOUR {
        plural(seconds / MINUTE, "minute")
    } else if seconds < DAY {
        plural(seconds / HOUR, "hour")
    } else if seconds < MONTH {
        plural(seconds / DAY, "day")
    } else if seconds < YEAR {
        plural(seconds / MONTH, "month")
    } else {
        plural(seconds / YEAR, "year")
    }
}

fn plural(count: u64, unit: &str) -> String {
    if count == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{count} {unit}s ago")
    }
}

struct Head {
    branch: String,
    refname: String,
    commit: String,
    detached: bool,
}

fn head(root: &Path) -> Result<Head, Failure> {
    let branch = git(root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let branch = branch.trim().to_owned();
    let refname = git(root, &["rev-parse", "--symbolic-full-name", "HEAD"])?;
    let refname = refname.trim().to_owned();
    let commit = git(root, &["rev-parse", "HEAD"])?;
    let commit = commit.trim().to_owned();
    // `rev-parse --abbrev-ref` answers `HEAD` exactly when no branch is checked out.
    let detached = branch == "HEAD";
    Ok(Head {
        branch,
        refname,
        commit,
        detached,
    })
}

fn short(commit: &str) -> &str {
    commit.get(..12).unwrap_or(commit)
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[derive(Default)]
struct Changes {
    staged: Vec<String>,
    unstaged: Vec<String>,
    untracked: Vec<String>,
}

fn changes(root: &Path) -> Result<Changes, Failure> {
    let status = git(root, &["status", "--porcelain=v1"])?;
    let mut changes = Changes::default();
    for line in status.lines() {
        let bytes = line.as_bytes();
        let (Some(&index), Some(&worktree)) = (bytes.first(), bytes.get(1)) else {
            continue;
        };
        if index == b'?' && worktree == b'?' {
            changes.untracked.push(line.to_owned());
            continue;
        }
        if index != b' ' {
            changes.staged.push(line.to_owned());
        }
        if worktree != b' ' {
            changes.unstaged.push(line.to_owned());
        }
    }
    Ok(changes)
}

fn print_group(label: &str, lines: &[String]) {
    say!("{label}: {}", lines.len());
    for line in lines {
        say!("  {line}");
    }
}

struct Remote {
    name: String,
    url: String,
}

/// Reads `git remote -v`, keeping one entry per name and URL: the verb git appends is about the
/// direction a URL is used in, not about the remote.
fn remotes(root: &Path) -> Result<Vec<Remote>, Failure> {
    let output = git(root, &["remote", "-v"])?;
    let mut remotes: Vec<Remote> = Vec::new();
    for line in output.lines() {
        let mut fields = line.split_whitespace();
        let (Some(name), Some(url)) = (fields.next(), fields.next()) else {
            continue;
        };
        let seen = remotes
            .iter()
            .any(|remote| remote.name == name && remote.url == url);
        if seen {
            continue;
        }
        remotes.push(Remote {
            name: name.to_owned(),
            url: url.to_owned(),
        });
    }
    Ok(remotes)
}

fn report_remotes(remotes: &[Remote]) {
    if remotes.is_empty() {
        say!("remotes: none");
    } else {
        say!("remotes:");
        for remote in remotes {
            say!("  {}  {}", remote.name, remote.url);
        }
    }

    let official: Vec<&str> = remotes
        .iter()
        .filter(|remote| normalize_remote(&remote.url) == OFFICIAL_REPOSITORY)
        .map(|remote| remote.name.as_str())
        .collect();
    if official.is_empty() {
        say!("official remote: none");
        say!("expected remote: https://github.com/rorolala-vcs/rorolala-vcs");
    } else {
        say!("official remote: {}", official.join(", "));
    }
}

/// Reduces a remote URL to `host/owner/name`, so the transport git was told to use does not decide
/// whether a clone points at the official repository.
fn normalize_remote(raw: &str) -> String {
    let raw = raw.trim();
    // A scp-like address has no scheme, and its colon separates the host from the path.
    let scp_like = !raw.contains("://");
    let rest = ["https://", "ssh://", "git://"]
        .into_iter()
        .find_map(|scheme| raw.strip_prefix(scheme))
        .unwrap_or(raw);
    let rest = rest
        .split_once('@')
        .map_or(rest, |(_, host_and_path)| host_and_path);

    let joined = if scp_like {
        rest.split_once(':')
            .map_or_else(|| rest.to_owned(), |(host, path)| format!("{host}/{path}"))
    } else {
        // A port is dropped: two clones of one repository can differ by the port they are reached
        // through, and that difference is not a different repository.
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let host = authority
            .split_once(':')
            .map_or(authority, |(host, _port)| host);
        if path.is_empty() {
            host.to_owned()
        } else {
            format!("{host}/{path}")
        }
    };

    let joined = joined.trim_end_matches('/');
    let joined = joined.strip_suffix(".git").unwrap_or(joined);
    joined.to_lowercase()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{OFFICIAL_REPOSITORY, format_age, normalize_remote};

    #[test]
    fn official_urls_normalize_to_the_official_repository() {
        for url in [
            "https://github.com/rorolala-vcs/rorolala-vcs",
            "https://github.com/rorolala-vcs/rorolala-vcs.git",
            "ssh://git@github.com/rorolala-vcs/rorolala-vcs.git",
            "git://github.com/rorolala-vcs/rorolala-vcs",
            "git@github.com:rorolala-vcs/rorolala-vcs.git",
            "git@github.com:rorolala-vcs/rorolala-vcs",
        ] {
            assert_eq!(normalize_remote(url), OFFICIAL_REPOSITORY, "{url}");
        }
    }

    #[test]
    fn other_repositories_do_not_normalize_to_the_official_one() {
        for url in [
            "https://github.com/someone/rorolala-vcs.git",
            "https://gitlab.com/rorolala-vcs/rorolala-vcs.git",
            "git@github.com:someone/rorolala-vcs.git",
        ] {
            assert_ne!(normalize_remote(url), OFFICIAL_REPOSITORY, "{url}");
        }
    }

    #[test]
    fn ages_use_the_largest_whole_unit() {
        assert_eq!(format_age(Duration::from_secs(0)), "just now");
        assert_eq!(format_age(Duration::from_secs(59)), "just now");
        assert_eq!(format_age(Duration::from_secs(60)), "1 minute ago");
        assert_eq!(format_age(Duration::from_secs(120)), "2 minutes ago");
        assert_eq!(format_age(Duration::from_secs(3_600)), "1 hour ago");
        assert_eq!(format_age(Duration::from_hours(24)), "1 day ago");
        assert_eq!(format_age(Duration::from_hours(720)), "1 month ago");
        assert_eq!(format_age(Duration::from_hours(8_760)), "1 year ago");
    }
}
