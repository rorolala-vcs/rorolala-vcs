//! Watching a run: what it says while it runs, and what becomes of it.
//!
//! A run says its progress down a channel — see [`rorolala_utils_progress`] — and this is
//! what reads that channel while the run goes on: `rola` was asked how the progress should
//! appear, and here is where that answer is carried out. Nothing is drawn on the thread the
//! work runs on, so watching a run never slows it down.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{IsTerminal as _, Write as _};
use std::thread::JoinHandle;
use std::time::Duration;

use rorolala_cli_setups::ResProgressSetting;
use rorolala_utils_progress::{
    Direction, Progress, Signal, Transmitter, fraction, spinner_frame, task_line, total_line,
};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

/// How many signals may be waiting to be drawn before the ones that do not fit are dropped.
///
/// Progress that has been overtaken is worth less than the work it describes, and a reader
/// this far behind has already been overtaken: the depth is what keeps a slow terminal from
/// becoming a slow run.
const DEPTH: usize = 256;

/// How long a reader waits between looks when the run has said nothing.
///
/// The wait is what a spinner turns on, and it is short enough that the turn is smooth and
/// long enough that a run that says nothing costs nothing.
const POLL: Duration = Duration::from_millis(80);

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

        let showing = match setting {
            ResProgressSetting::Jsonl => Showing::Records,
            _ => Showing::Terminal {
                drawn: 0,
                drawable: std::io::stderr().is_terminal(),
            },
        };
        let mut watcher = Watcher {
            showing,
            tick: 0,
            tasks: BTreeMap::new(),
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

/// A reader of a run's signals.
struct Watcher {
    /// How the run is being shown.
    showing: Showing,
    /// How many times the spinner has turned, which is what makes it turn.
    tick: usize,
    /// What has been said about each task, by the id the task is named by.
    ///
    /// Ids are handed out in the order tasks begin, so the map's own order is the order they
    /// were begun in, which is the order a reader wants them drawn in.
    tasks: BTreeMap<u64, Said>,
}

/// How a run's signals are shown.
enum Showing {
    /// The lines are redrawn in place on a terminal.
    Terminal {
        /// How many lines are on the terminal right now.
        drawn: usize,
        /// Whether what is being written to is a terminal at all.
        ///
        /// Nothing is redrawn when it is not: the control sequences that move a cursor mean
        /// nothing to a file or a pipe, and writing them there would be noise in the record
        /// of what the run said rather than the record itself.
        drawable: bool,
    },
    /// One record per signal, for another program to read.
    Records,
}

impl Watcher {
    /// Reads `receiver` until the run has nothing more to say.
    fn watch(&mut self, receiver: &mut mpsc::Receiver<Signal>) {
        loop {
            match receiver.try_recv() {
                Ok(signal) => self.take(&signal),
                Err(TryRecvError::Empty) => {
                    self.tick = self.tick.wrapping_add(1);
                    self.redraw();
                    std::thread::sleep(POLL);
                }
                Err(TryRecvError::Disconnected) => break,
            }
        }

        // What is left on the terminal is the run's, not the reader's: it is cleared before
        // the reader ends so that whatever is said after the run lands on a clean line.
        self.erase();
    }

    /// Takes one thing the run said.
    fn take(&mut self, signal: &Signal) {
        if matches!(self.showing, Showing::Records) {
            let _ = Self::write_record(signal);

            return;
        }

        self.apply(signal);
        self.redraw();
    }

    /// Writes `signal` as one JSON record.
    fn write_record(signal: &Signal) -> std::io::Result<()> {
        let record = serde_json::to_string(signal).unwrap_or_default();
        let mut stderr = std::io::stderr().lock();
        writeln!(stderr, "{record}")?;
        stderr.flush()
    }

    /// Reads `signal` into what is known of the tasks.
    fn apply(&mut self, signal: &Signal) {
        match signal {
            Signal::Begin {
                id,
                parent,
                what,
                direction,
                total,
            } => {
                self.tasks.insert(
                    *id,
                    Said {
                        parent: *parent,
                        what: what.clone(),
                        direction: *direction,
                        total: *total,
                        done: 0,
                        doing: None,
                    },
                );

                // A started task with no direction of its own names the piece of the task
                // above it being done right now, rather than being a line of its own.
                if let (Some(parent), None) = (parent, direction)
                    && let Some(above) = self.tasks.get_mut(parent)
                {
                    above.doing = Some((*id, what.clone()));
                }
            }
            Signal::Advance { id, done } => {
                if let Some(said) = self.tasks.get_mut(id)
                    && said.direction.is_some()
                {
                    said.done = *done;
                }
            }
            Signal::Finish { id } => {
                let Some(said) = self.tasks.remove(id) else {
                    return;
                };

                // A piece that is done is no longer being done, so the name it gave the task
                // above it is given back — unless something else has taken its place.
                if said.direction.is_none()
                    && let Some(parent) = said.parent
                    && let Some(above) = self.tasks.get_mut(&parent)
                    && above.doing.as_ref().is_some_and(|(held, _)| *held == *id)
                {
                    above.doing = None;
                }
            }
        }
    }

    /// The lines the tasks are drawn as, in the order they were begun in.
    fn lines(&self) -> Vec<String> {
        let spinner = spinner_frame(self.tick);

        self.tasks
            .values()
            .filter_map(|said| match said.direction {
                Some(direction) => {
                    let doing = said.doing.as_ref().map_or("", |(_, doing)| doing.as_str());

                    Some(task_line(
                        spinner,
                        &said.what,
                        direction,
                        fraction(said.done, said.total),
                        doing,
                    ))
                }
                // The task above every other one is the whole of the run, and a task with no
                // direction below it is not a line of its own.
                None if said.parent.is_none() => {
                    Some(total_line(spinner, fraction(said.done, said.total)))
                }
                None => None,
            })
            .collect()
    }

    /// Shows the tasks as they are now.
    fn redraw(&mut self) {
        if !matches!(self.showing, Showing::Terminal { drawable: true, .. }) {
            return;
        }

        let lines = self.lines();
        let Showing::Terminal { drawn, .. } = &mut self.showing else {
            return;
        };

        // The block is as tall as the taller of what was there and what is being drawn, since
        // the lines that are gone still have to be written over.
        let rows = (*drawn).max(lines.len());
        draw(*drawn, &lines);
        *drawn = rows;
    }

    /// Takes the lines off the terminal.
    fn erase(&mut self) {
        let Showing::Terminal { drawn, drawable } = &mut self.showing else {
            return;
        };

        if !*drawable || *drawn == 0 {
            return;
        }

        draw(*drawn, &[]);
        *drawn = 0;
    }
}

/// What has been said about one task.
struct Said {
    /// The task this one is part of, when it is part of another one.
    parent: Option<u64>,
    /// What the task is called.
    what: String,
    /// Which way its work moves, when it moves any.
    direction: Option<Direction>,
    /// How much of the task there is in all, when that is known.
    total: Option<u64>,
    /// How much of the task is done.
    done: u64,
    /// The piece being done right now, and the task that named it.
    doing: Option<(u64, String)>,
}

/// Draws `lines` over the `drawn` lines already on the terminal, and leaves the cursor where
/// the lines begin.
///
/// A line is redrawn rather than appended to, so what a reader watches is a block that stays
/// where it is rather than a scroll that grows: the block is written over itself, and the
/// cursor is put back at its first line so the next drawing can write over it again. Lines
/// that are no longer there are cleared, so a block that shrinks leaves nothing behind it.
fn draw(drawn: usize, lines: &[String]) {
    let rows = drawn.max(lines.len());
    let mut out = String::from("\r");

    if drawn > 0 {
        // Back to the first of the lines already there; there is nowhere to go when there are
        // none, since the cursor is where the block would begin.
        let _ = write!(out, "\x1b[{drawn}A");
    }

    for at in 0..rows {
        if at > 0 {
            out.push('\n');
        }

        // Each line is cleared before it is written, so a shorter line does not leave the tail
        // of a longer one behind it.
        out.push_str("\x1b[2K");
        if let Some(line) = lines.get(at) {
            out.push_str(line);
        }
    }

    if rows > 1 {
        let _ = write!(out, "\x1b[{}A", rows - 1);
    }

    let mut stderr = std::io::stderr().lock();
    let _ = stderr.write_all(out.as_bytes());
    let _ = stderr.flush();
}
