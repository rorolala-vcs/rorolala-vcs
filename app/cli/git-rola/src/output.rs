//! Writing a run's answer out, and handing it to a pager when git would.
//!
//! A renderer draws into mingling's buffer, and the framework writes that buffer out once the run
//! is over. Both streams are taken here in one `post_render` hook, which is the last place the
//! answer is whole and nothing has been written yet, and written with the pager git would have
//! chosen: `pager.rola`, then `GIT_PAGER`, then `core.pager`, then `PAGER`, then `less`, with an
//! empty value or `cat` meaning that nothing is paged.
//!
//! git's own pager reading is not copied, it is asked for: `git var GIT_PAGER` answers the chain,
//! `GIT_CONFIG_PARAMETERS` included, so `git -c core.pager=less rola check` pages where git would.
//!
//! Both streams are written, errors first, because a failure line is written before the result it
//! is about. The buffer keeps each stream's own order; the interleaving of the two is lost by the
//! framework's shape, which hands out one stream at a time, and every renderer here writes its
//! failures before its result anyway.
//!
//! A pager is a convenience, not a contract. One that cannot be started, or that stops reading, is
//! not a failure of the run: the answer is written out instead and the status the renderer chose is
//! left alone, which is the one place this program is kinder than git (`fatal: unable to execute
//! pager`).

use std::io::{IsTerminal as _, Write as _};
use std::process::{Child, Command, Stdio};

use mingling::{ProgramCollect, hook::ProgramHook, setup::ProgramSetup};
use rust_i18n::t;

use crate::exit_codes::EC_ERR;
use crate::git_config;

/// What a pager set to this means: write the text out as it is.
const NO_PAGER: &str = "cat";

/// A [`ProgramSetup`] that writes every answer out, through a pager when git would use one.
pub struct OutputSetup;

impl<C: ProgramCollect<Enum = C> + 'static> ProgramSetup<C> for OutputSetup {
    fn setup(self, program: &mut mingling::Program<C>) {
        program.with_hook(ProgramHook::empty().on_post_render(|info| {
            // Errors first, then the result. Taking a stream out leaves the other in the buffer,
            // and the framework's own write of it then has nothing left to write.
            let stderr_text = info.result.take_stderr().concat();
            let stdout_text = info.result.take_stdout().concat();

            write_answer(&stdout_text, &stderr_text);
        }));
    }
}

/// Writes both streams out, paged when the run has a pager and a terminal to page on.
fn write_answer(stdout_text: &str, stderr_text: &str) {
    let Some(command) = pager() else {
        write_apart(stdout_text, stderr_text);

        return;
    };

    // git puts stderr into the pager only when it is itself a terminal: a redirected stderr stays
    // on the file it was pointed at, and is written there before the pager is started.
    let errors_paged = std::io::stderr().is_terminal();

    if !errors_paged {
        write_stderr(stderr_text);
    }

    if page(&command, stdout_text, stderr_text, errors_paged) {
        return;
    }

    // Nothing has been written to the pager that could be read, so the answer is written out. When
    // stderr was already written it is not written a second time.
    if errors_paged {
        write_apart(stdout_text, stderr_text);
    } else {
        write_stdout(stdout_text);
    }
}

/// The pager this run should use, if it should page at all.
fn pager() -> Option<String> {
    // A pipe or a file is another program or a person reading, and a program cannot page a page.
    if !std::io::stdout().is_terminal() {
        return None;
    }

    // git sets this when it started a pager for this command already; a second one would page the
    // pager's own screen.
    if pager_in_use() {
        return None;
    }

    let configured = git_config::get("pager.rola");
    let command = match configured.as_deref().map(str::trim) {
        // git reads `pager.<cmd>` as a boolean first: false means no pager, true means the usual
        // chain, and anything else is the command itself.
        Some("false") => return None,
        Some("true") | None => git_config::var("GIT_PAGER"),
        Some(configured) => match std::env::var("GIT_PAGER") {
            Ok(pager) if !pager.is_empty() => Some(pager),
            _ => Some(configured.to_owned()),
        },
    }?;

    let command = command.trim();

    if command.is_empty() || command == NO_PAGER {
        return None;
    }

    Some(command.to_owned())
}

/// Whether a pager is already running for this run.
fn pager_in_use() -> bool {
    std::env::var("GIT_PAGER_IN_USE").is_ok_and(|value| truthy(&value))
}

/// Whether an environment value says yes.
///
/// git reads `GIT_PAGER_IN_USE` with its boolean reader, so the words that mean no mean no here.
fn truthy(value: &str) -> bool {
    !value.is_empty()
        && !matches!(
            value.to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        )
}

/// Hands the answer to `command`, and says whether it got there.
fn page(command: &str, stdout_text: &str, stderr_text: &str, errors_paged: bool) -> bool {
    let Ok(mut child) = spawn(command) else {
        return false;
    };

    let written = child.stdin.take().is_some_and(|mut into_pager| {
        let mut answer = String::with_capacity(stderr_text.len() + stdout_text.len());

        if errors_paged {
            answer.push_str(stderr_text);
        }

        answer.push_str(stdout_text);

        into_pager
            .write_all(answer.as_bytes())
            .and_then(|()| into_pager.flush())
            .is_ok()
    });

    // What the pager exits with is the pager's own business, quitting before the end included: the
    // answer has been handed over, and what became of it is what a reader of a pager expects.
    let _ = child.wait();

    written
}

/// Starts `command` as git starts a pager: through the shell, reading the answer on its input.
fn spawn(command: &str) -> std::io::Result<Child> {
    let mut process = Command::new("sh");

    process
        .arg("-c")
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        // A git the pager runs is inside a pager already, and must not page a page.
        .env("GIT_PAGER_IN_USE", "true");

    // The pager reads these as its own settings. git supplies them only when the environment has
    // not, so a person's own choice is left as it is.
    if std::env::var_os("LESS").is_none() {
        process.env("LESS", "FRX");
    }

    if std::env::var_os("LV").is_none() {
        process.env("LV", "-c");
    }

    process.spawn()
}

/// Writes each stream where it belongs, errors first.
fn write_apart(stdout_text: &str, stderr_text: &str) {
    write_stderr(stderr_text);
    write_stdout(stdout_text);
}

/// Writes `text` to standard error, leaving a failure to the reader, as git does.
fn write_stderr(text: &str) {
    let mut err = std::io::stderr().lock();
    let _ = err.write_all(text.as_bytes());
    let _ = err.flush();
}

/// Writes `text` to standard output, and ends the run on git's write failure when it cannot.
fn write_stdout(text: &str) {
    let mut out = std::io::stdout().lock();

    if out
        .write_all(text.as_bytes())
        .and_then(|()| out.flush())
        .is_err()
    {
        // On Unix, `SIGPIPE` has already ended the process here, so this is the platform that
        // reports a broken pipe to the write, or a full disk. git ends such a run with 128.
        write_stderr(&format!(
            "{}{}\n",
            t!("message.fatal"),
            t!("git_rola.err_write_stdout")
        ));

        std::process::exit(EC_ERR);
    }
}

#[cfg(test)]
mod tests {
    use super::truthy;

    #[test]
    fn a_pager_in_use_is_read_the_way_a_boolean_is() {
        assert!(truthy("true"));
        assert!(truthy("1"));
        assert!(truthy("yes"));
        assert!(!truthy(""));
        assert!(!truthy("0"));
        assert!(!truthy("false"));
    }
}
