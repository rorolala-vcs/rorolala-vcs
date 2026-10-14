//! Handing a long answer to a pager, so that reading it does not mean losing its head.
//!
//! A run draws its answer into the render buffer, and the framework writes that buffer out once
//! the run is over. What is drawn is the contract, so a pager cannot be a different way of
//! drawing: it is the same text, handed to something that shows a screenful of it at a time.
//! That is what this registers — a `post_render` hook, which is the last place the buffer is
//! whole and nothing has been written out yet. The hook takes the answer's entries out of the
//! buffer, so the framework writes none of them again, and writes them itself: through a pager
//! when there is a terminal to page on, and straight out when there is not.
//!
//! Which stream the answer was drawn on is the run's to say: a command draws its result on
//! `stdout`, and a help answer on `stderr`, which is where the help renderers write it. The hook
//! takes that stream's entries and leaves the other where it is, in the order it was written — a
//! command's warnings, progress and failures stay on `stderr` for the framework to write, exactly
//! as they always did. So `rola ... > file` still writes the answer to the file and still says its
//! own troubles on the terminal, `rola ... | head` is still a pipe, and a help answer long enough
//! to lose its head is paged rather than left to scroll away.
//!
//! Three things have to hold before anything is paged, and each of them is a way a run is read
//! rather than watched:
//!
//! - the run did not ask for no pager (`--no-pager`);
//! - `stdout` is a terminal. A pipe or a redirect is another program or a file reading, and a
//!   program cannot page a page;
//! - there is more of it than the terminal can show at once. Answers that fit are written out
//!   directly, since a pager for four lines is a keypress for nothing.
//!
//! Which pager it is, is the machine's to say first: `$PAGER` names one, and `cat` means the
//! machine has asked for the text to be written out as it is. With nothing named, the pager
//! carried inside the program is used — see [`minus`] — so a machine with no pager installed can
//! still read a long answer.
//!
//! A pager is a convenience, not a contract. If one cannot be started, or stops reading, or dies,
//! the answer is written out rather than lost, and the run ends with the exit code and the streams
//! it would have had: only what the pager does with the text is the pager's business.

use std::io::{IsTerminal as _, Write};
use std::process::{Command, Stdio};

use mingling::{
    ProgramCollect, RenderResult, config::QueryMode, hook::ProgramHook, setup::ProgramSetup,
};
use rorolala_cli_setups::ResNoPager;

/// The pager a machine names with `$PAGER` when it wants none: the text is to be written out as it
/// is, which is what this program does when there is no pager at all.
const NO_PAGER: &str = "cat";

/// How many rows a terminal is taken to have when its size cannot be read.
///
/// A terminal that will not say how tall it is is not one to guess tall for: too few rows only
/// means a pager is offered where the text would have fitted, and pressing a key is a smaller
/// price than losing the head of an answer that did not fit.
const ASSUMED_ROWS: usize = 24;

/// A [`ProgramSetup`] implementation that hands a long answer to a pager.
///
/// Nothing about a command changes: a command draws its result exactly as it always did, and the
/// result is paged on the way out. Its help is drawn the same way and paged the same way, so a
/// command knows nothing of this either.
pub struct PagerSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for PagerSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram> + 'static,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        program.with_hook(ProgramHook::empty().on_post_render(|info| {
            // Read here rather than while the setup runs: a resource is reachable through the
            // program, and the program is only in place once the run has begun. Read at setup
            // time it is not there at all.
            if !no_pager::<ThisProgram>() {
                page(info.result, info.mode);
            }
        }));
    }
}

/// Whether the run asked for no pager.
///
/// The answer is a resource rather than a flag, because the flag is global: `--no-pager` is picked
/// out of the whole argument list before any command sees it. Asking the program for it is the only
/// way left to read it, and it is read the way the framework's own hooks read a resource.
fn no_pager<ThisProgram>() -> bool
where
    ThisProgram: ProgramCollect<Enum = ThisProgram> + 'static,
{
    mingling::this::<ThisProgram>().modify_res(|held: &mut ResNoPager| **held)
}

/// Which stream an answer was drawn on.
///
/// A command draws its result on `stdout`, and a help answer on `stderr` — which is where the help
/// renderers write it. The pager reads, and when it cannot page, writes back that one stream, so
/// whatever a run says on the other is left exactly where it was.
#[derive(Clone, Copy)]
enum Answer {
    /// A command's result, drawn on `stdout`.
    Result,

    /// A help answer, drawn on `stderr`.
    Help,
}

impl Answer {
    /// The stream a render of `mode` draws its answer on.
    const fn of(mode: QueryMode) -> Self {
        match mode {
            QueryMode::Usage => Self::Help,
            QueryMode::None | QueryMode::Version => Self::Result,
        }
    }

    /// Whether the answer is being written to a terminal, which is what there is to page on.
    fn is_terminal(self) -> bool {
        match self {
            Self::Result => std::io::stdout().is_terminal(),
            Self::Help => std::io::stderr().is_terminal(),
        }
    }

    /// The answer's entries, in the order the buffer holds them.
    fn read(self, result: &RenderResult) -> Vec<&String> {
        match self {
            Self::Result => result.read_stdout(),
            Self::Help => result.read_stderr(),
        }
    }

    /// Takes the answer's entries out of the buffer, leaving the other stream's in place.
    fn take(self, result: &mut RenderResult) -> Vec<String> {
        match self {
            Self::Result => result.take_stdout(),
            Self::Help => result.take_stderr(),
        }
    }

    /// Writes the answer out where it was drawn, the way the framework would have written it.
    fn write(self, text: &[u8]) {
        match self {
            Self::Result => write_out(&mut std::io::stdout().lock(), text),
            Self::Help => write_out(&mut std::io::stderr().lock(), text),
        }
    }
}

/// Pages the answer `result` holds, if it is one to page at all.
fn page(result: &mut RenderResult, mode: QueryMode) {
    let answer = Answer::of(mode);

    // Two terminals are asked about first, and both have to be one. The pager draws on `stdout`,
    // so a run whose `stdout` is a pipe or a file has nothing to page on. The answer is on its own
    // stream, so a run whose answer is being read — a result piped elsewhere, a help answer sent to
    // a file — has nothing to page either. Asking both keeps an answer on the stream it was drawn
    // on: a help answer is never moved onto `stdout`, and a result is never moved off it.
    if !answer.is_terminal() || !std::io::stdout().is_terminal() {
        return;
    }

    if line_count(&answer.read(result)) <= rows() {
        return;
    }

    // The answer is taken out of the buffer only once it is settled that this run is paging it:
    // taken and not written, it is lost, and every way out above is a way out with the buffer whole
    // for the framework to write.
    let text = merge(&answer.take(result)).into_bytes();

    if let Ok(pager) = std::env::var("PAGER") {
        let pager = pager.trim();

        // The machine named a pager of its own — one that pages, or one that says the text is not
        // to be paged at all.
        if !pager.is_empty() {
            if pager == NO_PAGER {
                answer.write(&text);
            } else if !through(pager, &text) {
                // A pager that cannot be reached or stops reading has not shown the answer, so it
                // is written out instead. `less` quits once there is nothing more to read, which
                // is a write this run has no reason to treat as a failure — and on Unix the same
                // quit arrives as `SIGPIPE`, which is deliberately fatal (see `main.rs`), so this
                // is the path Windows takes.
                answer.write(&text);
            }

            return;
        }
    }

    // The machine named no pager, so the one carried inside this program is used. What it makes of
    // the terminal is reported by the pager itself at the moment it fails; saying it again here
    // would be one more line on a screen that may not have been given back.
    if page_with_minus(&text).is_err() {
        answer.write(&text);
    }
}

/// How many lines the answer holds, as the buffer holds it.
///
/// A buffered line is what the framework writes next to what it writes it with, so the entries are
/// read the way the framework would write them: in order, one after another — see [`merge`]. A
/// line is ended by a newline, and an answer whose last line has none is still a line.
fn line_count<S: AsRef<str>>(entries: &[S]) -> usize {
    let newlines: usize = entries
        .iter()
        .map(|entry| entry.as_ref().matches('\n').count())
        .sum();

    let Some(last) = entries.last() else {
        return 0;
    };

    newlines + usize::from(!last.as_ref().ends_with('\n'))
}

/// The answer as one string, in the order the buffer holds it.
///
/// The entries are written one after another and nothing is put between them, which is exactly what
/// [`RenderResult::std_print`](mingling::RenderResult::std_print) would have done: a buffered line
/// is one entry and the newline that ends it is the next, so joining them with anything of this
/// own would draw the answer differently from the way it was drawn.
fn merge<S: AsRef<str>>(entries: &[S]) -> String {
    entries.iter().map(AsRef::as_ref).collect()
}

/// How many rows the terminal this run writes to has, as far as it will say.
fn rows() -> usize {
    console::Term::stdout()
        .size_checked()
        .map_or(ASSUMED_ROWS, |(rows, _columns)| usize::from(rows))
}

/// Writes the answer out where the run was made, the way the framework would have written it.
fn write_out(stream: &mut impl Write, answer: &[u8]) {
    let _ = stream.write_all(answer);
    let _ = stream.flush();
}

/// Writes the answer through the pager `program` names, and says whether it got there.
///
/// The answer is written to the pager's own input rather than to the terminal, so the pager is what
/// draws: this is the arrangement every program that pages makes. A pager that has gone stops
/// reading, and a write to a pipe nobody reads is not a failure of this run.
fn through(program: &str, answer: &[u8]) -> bool {
    let Ok(mut child) = Command::new(program).stdin(Stdio::piped()).spawn() else {
        return false;
    };

    let written = child.stdin.take().is_some_and(|mut into_pager| {
        into_pager
            .write_all(answer)
            .and_then(|()| into_pager.flush())
            .is_ok()
    });

    // What the pager exited with is the pager's own business: the answer has been handed over, and
    // what became of it is what a reader of a pager expects — quitting before the end included.
    let _ = child.wait();

    written
}

/// Pages the answer with the pager carried inside the program.
///
/// Nothing tells the pager to end the process when it is quit, which is what is wanted: a run that
/// paged its answer still has an exit code to end with and still has whatever it kept to write
/// back, and neither of those is the pager's to decide.
fn page_with_minus(answer: &[u8]) -> Result<(), minus::MinusError> {
    let pager = minus::Pager::new();

    // The pager is handed the text as it was drawn, so what it is given is what a terminal would
    // have been given.
    pager.set_text(String::from_utf8_lossy(answer).into_owned())?;

    minus::page_all(pager)
}

#[cfg(test)]
mod tests {
    use super::{Answer, QueryMode, line_count, merge, through};

    /// How many lines the test answer is.
    const LINES: usize = 40;

    /// A result holding `LINES` numbered lines, drawn the way a renderer draws one.
    fn answer() -> mingling::RenderResult {
        let mut result = mingling::RenderResult::new();

        for number in 0..LINES {
            result.println(format!("line {number}"));
        }

        result
    }

    #[test]
    fn an_answer_is_merged_the_way_the_framework_would_have_written_it() {
        let mut result = answer();
        let lines = line_count(&result.read_stdout());
        let merged = merge(&result.take_stdout());

        // A buffered line and the newline that ends it are two entries, so what is written ends
        // every line, the last included.
        assert!(merged.ends_with("line 39\n"));
        assert_eq!(lines, LINES);
        assert_eq!(merged.lines().count(), LINES);
        assert_eq!(merge::<String>(&[]), "");
    }

    #[test]
    fn a_line_that_is_print_holds_its_own_newlines_and_is_counted_by_them() {
        let mut result = mingling::RenderResult::new();
        let text = "first\nsecond\nthird\n";

        result.print(text);

        assert_eq!(line_count(&result.read_stdout()), 3);
        assert_eq!(merge(&result.take_stdout()), text);
    }

    #[test]
    fn an_answer_is_read_from_the_stream_it_was_drawn_on() {
        let mut result = mingling::RenderResult::new();
        result.println("a result");
        result.eprintln("a help answer");

        // A command's answer is on `stdout`; a help answer is on `stderr`.
        assert_eq!(
            merge(&Answer::of(QueryMode::None).read(&result)),
            "a result\n"
        );
        assert_eq!(
            merge(&Answer::of(QueryMode::Usage).read(&result)),
            "a help answer\n"
        );
    }

    /// A pager that reads its input and counts it, taken from the command shell.
    ///
    /// The shell is the one program every test machine has, and what it is asked for is a count of
    /// the lines it was handed: what is being checked is that the answer arrives whole, and that a
    /// pager which took it and left is not reported as a failure.
    ///
    /// This is `#[ignore]`d because the sandbox these tests are run in under the gate gives no
    /// machine here the right to start a program: it fails with `Access is denied`, which says
    /// nothing about the pager. Run it with `cargo test -p rola -- --ignored`.
    #[test]
    #[cfg(windows)]
    #[ignore = "starting a program is what the test sandbox refuses"]
    fn a_pager_is_handed_the_whole_answer() {
        let counted = std::env::temp_dir().join("rorolala-pager-counted.txt");
        let _ = std::fs::remove_file(&counted);

        let command = format!("find /c /v \"\" > \"{}\"", counted.display());

        assert!(through(
            &format!("{SHELL} /c {command}"),
            merge(&answer().take_stdout()).as_bytes()
        ));

        // `find /c` counts the lines it was given, so its own count is what says the answer arrived
        // whole: forty lines, the newline the framework would have written included.
        let written = std::fs::read_to_string(&counted).unwrap();
        assert!(written.contains("40"), "{written}");
    }

    /// The shell the counting pager is asked for, which is what a machine here always has.
    #[cfg(windows)]
    const SHELL: &str = "cmd.exe";

    #[test]
    fn a_pager_that_is_not_there_is_reported_rather_than_assumed() {
        // The answer is written out instead, so what a run must not do is claim the pager took it.
        assert!(!through("rola-no-such-pager-exists", b"anything\n"));
    }
}
