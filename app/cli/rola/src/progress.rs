//! Watching a run: what it says while it runs, and what becomes of it.
//!
//! A run says its progress down a channel — see [`rorolala_utils_progress`] — and this is what
//! reads that channel while the run goes on: `rola` was asked how the progress should appear,
//! and here is where that answer is carried out. Nothing is drawn on the thread the work runs
//! on, so watching a run never slows it down.
//!
//! The bars are [`indicatif`]'s, drawn the way Rorolala's build tooling draws its own: a task
//! is one bar, the tasks of a run are stacked under it, and the bar carries how far it has got
//! rather than a picture of it. The drawing is indicatif's to do and nobody else's, which is
//! what keeps a run of this program looking like every other run of a Rust program.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::thread::JoinHandle;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rorolala_cli_setups::ResProgressSetting;
use rorolala_utils_progress::{Direction, Progress, Signal, Transmitter};
use tokio::sync::mpsc;

/// How many signals may be waiting to be drawn before the ones that do not fit are dropped.
///
/// Progress that has been overtaken is worth less than the work it describes, and a reader
/// this far behind has already been overtaken: the depth is what keeps a slow terminal from
/// becoming a slow run.
const DEPTH: usize = 256;

/// The characters a bar is drawn with: what is behind the work, where the work is, and what is
/// ahead of it.
const CHARS: &str = "=> ";

/// The bar the whole of a run is drawn as.
///
/// The width is written into the template rather than named, since a template's widths are
/// literal: the two bars below are 28 cells for the same reason — so that a task's bar begins
/// where every other bar of the run does.
const WHOLE_TEMPLATE: &str = "  [{bar:28}] {pos}/{len}";

/// The bar one task of a run is drawn as: which way its work moves, how far it has got, and
/// what it is moving right now.
const TASK_TEMPLATE: &str = "{prefix} [{bar:28}] {pos}/{len}: {msg}";

/// How much of the name of a piece being moved is shown.
///
/// A piece is named by the key its content is kept under, and a whole key is sixty-four
/// characters: shown in full it is longer than the line it has to fit on, so it is the head
/// of it that is shown — enough of it to tell one piece from the next, which is all a reader
/// is being shown it for.
const PIECE: usize = 7;

/// What is written where the rest of a piece's name was.
const REST: &str = "...";

/// A run's progress, being said to whoever was asked to hear it.
///
/// The reader runs on a thread of its own, so it keeps up with the run without the run ever
/// waiting for it. [`finish`](Self::finish) is what ends it: it lets go of the last way to
/// say anything, waits for the reader to draw what is left, and leaves the terminal as it
/// was. A run that forgets to finish leaks the thread until the program ends.
pub struct Reporting {
    progress: Progress,
    watching: Option<JoinHandle<()>>,
}

impl Reporting {
    /// Starts saying a run's progress the way `setting` asks.
    ///
    /// A run that asked to say nothing is given [`Progress::silent`], which is the same
    /// progress as any other as far as the work is concerned and draws nothing at all.
    #[must_use]
    pub fn start(setting: ResProgressSetting) -> Self {
        if setting == ResProgressSetting::No {
            return Self {
                progress: Progress::silent(),
                watching: None,
            };
        }

        // The channel is made here and handed on as a `Progress`, so what the run is given
        // says nothing about where what it says ends up.
        let (transmitter, mut receiver) = Transmitter::channel(DEPTH);
        let progress = Progress::to(transmitter);

        let mut watcher = match setting {
            ResProgressSetting::Jsonl => Watcher::Records,
            _ => Watcher::Terminal(Terminal::new()),
        };

        let watching = Some(std::thread::spawn(move || {
            watcher.watch(&mut receiver);
        }));

        Self { progress, watching }
    }

    /// Where the run says what it is doing while it does it.
    #[must_use]
    pub fn progress(&self) -> Progress {
        self.progress.clone()
    }

    /// Ends the watching, once there is nothing more for the run to say.
    ///
    /// The way to say anything is let go of before the reader is waited for, because that is
    /// what the reader is waiting for: a run that held on to it would be waiting on a thread
    /// that is waiting on the run.
    pub fn finish(self) {
        let Self { progress, watching } = self;
        drop(progress);

        if let Some(watching) = watching {
            let _ = watching.join();
        }
    }
}

/// How a run's signals are shown.
enum Watcher {
    /// Bars on the terminal, a task at a time.
    Terminal(Terminal),
    /// One record per signal, for another program to read.
    Records,
}

impl Watcher {
    /// Reads `receiver` until the run has nothing more to say.
    ///
    /// The reader is a thread of its own and not a task, so it waits the way a thread waits:
    /// nothing here polls, and a run that says nothing costs nothing.
    fn watch(&mut self, receiver: &mut mpsc::Receiver<Signal>) {
        while let Some(signal) = receiver.blocking_recv() {
            match self {
                Self::Terminal(terminal) => terminal.take(&signal),
                Self::Records => Self::write_record(&signal),
            }
        }
    }

    /// Writes `signal` as one JSON record.
    ///
    /// What is written is a record rather than a picture, so it goes out whether or not
    /// anything is being watched by a person: the reader here is another program, and a
    /// terminal being unattended says nothing about that one.
    fn write_record(signal: &Signal) {
        let record = serde_json::to_string(signal).unwrap_or_default();
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(stderr, "{record}");
        let _ = stderr.flush();
    }
}

/// The bars a run is drawn as, stacked one under another.
struct Terminal {
    /// The bars, kept together so that they share the terminal and are redrawn as one block.
    bars: MultiProgress,
    /// What each bar is, by the id the task is named by.
    drawn: BTreeMap<u64, ProgressBar>,
}

impl Terminal {
    /// A terminal with nothing drawn on it yet.
    fn new() -> Self {
        Self {
            bars: MultiProgress::new(),
            drawn: BTreeMap::new(),
        }
    }

    /// Takes one thing the run said.
    fn take(&mut self, signal: &Signal) {
        match signal {
            Signal::Begin {
                id,
                parent,
                what,
                direction,
                total,
            } => self.begin(*id, *parent, what, *direction, *total),
            Signal::Advance { id, done } => {
                if let Some(bar) = self.drawn.get(id) {
                    bar.set_position(*done);
                }
            }
            Signal::Finish { id } => self.finish(*id),
        }
    }

    /// Starts drawing a task: a bar of its own when it is one, and a name on the bar above it
    /// when it is a piece of one.
    fn begin(
        &mut self,
        id: u64,
        parent: Option<u64>,
        what: &str,
        direction: Option<Direction>,
        total: Option<u64>,
    ) {
        match (parent, direction) {
            // The whole of the run: the bar every other bar is stacked under.
            (None, _) => {
                let bar = self.bars.add(bar(total, WHOLE_TEMPLATE));
                self.drawn.insert(id, bar);
            }
            // One direction of it: a bar of its own, saying which way its work moves.
            (Some(_), Some(direction)) => {
                let bar = self.bars.add(bar(total, TASK_TEMPLATE));
                bar.set_prefix(arrow(direction).to_string());

                if !what.is_empty() {
                    bar.set_message(what.to_owned());
                }

                self.drawn.insert(id, bar);
            }
            // A piece being done right now: not a bar of its own, but what the bar above it is
            // moving — which is the name at the end of that bar.
            (Some(parent), None) => {
                if let Some(above) = self.drawn.get(&parent) {
                    above.set_message(head(what));
                }
            }
        }
    }

    /// Stops drawing a task, and takes its bar off the terminal.
    ///
    /// A task that is over has nothing left to say, so what is left behind is the whole of the
    /// run rather than a row of bars that have all stopped: a piece being done is not a bar at
    /// all, so finishing one says nothing to the terminal and waits for the next name.
    fn finish(&mut self, id: u64) {
        if let Some(bar) = self.drawn.remove(&id) {
            bar.finish_and_clear();
        }
    }
}

/// The arrow a task's bar is prefixed with, which is what says which way its work moves.
const fn arrow(direction: Direction) -> char {
    match direction {
        Direction::Up => '↑',
        Direction::Down => '↓',
    }
}

/// The head of the name of a piece being moved, and what says the rest of it is not shown.
///
/// A name shorter than the head is shown whole, and so is one the same length: there is
/// nothing being left out of either, and a name that says everything it has to say should not
/// be made to look as though it does not.
fn head(what: &str) -> String {
    let mut shown: String = what.chars().take(PIECE).collect();

    if what.chars().count() > PIECE {
        shown.push_str(REST);
    }

    shown
}

/// A bar `total` long, drawn with `template`.
///
/// A task whose length is not known is drawn as an empty bar rather than a spinner: what this
/// program says about its work is how much of it there is, and a task that cannot say that is
/// a task nothing here knows how to draw.
fn bar(total: Option<u64>, template: &str) -> ProgressBar {
    let bar = ProgressBar::new(total.unwrap_or(0));

    // UNWRAP: both templates are this program's own, so a template that will not parse is a
    // mistake in the program rather than anything a run can bring about.
    bar.set_style(
        ProgressStyle::default_bar()
            .template(template)
            .unwrap()
            .progress_chars(CHARS),
    );

    bar
}

#[cfg(test)]
mod tests {
    use super::head;

    #[test]
    fn a_key_is_shown_by_its_head_and_says_the_rest_is_left_out() {
        assert_eq!(
            head("5b4ac19152456173022e06a1614665a594c615ce1307d40f9a3d5006622991e4"),
            "5b4ac19..."
        );
    }

    #[test]
    fn a_name_that_says_everything_it_has_to_say_is_shown_whole() {
        assert_eq!(head("a key"), "a key");
        assert_eq!(head("1234567"), "1234567");
        assert_eq!(head(""), "");
    }
}
