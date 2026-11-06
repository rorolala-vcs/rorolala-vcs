//! Watching a run: what it says while it runs, and what becomes of it.
//!
//! A run says its progress down a channel, see [`rorolala_utils_progress`], and this is what
//! reads that channel while the run goes on: `rola` was asked how the progress should appear,
//! and here is where that answer is carried out. Nothing is drawn on the thread the work runs
//! on, so watching a run never slows it down.
//!
//! A task is one row, and every row keeps the same columns: a spinner, a fixed-width label, a
//! line drawn in `━`, `╸` and `─`, a percentage, and the piece being moved. A task that does
//! not know its length has no percentage to show, so its line becomes a marquee that runs right
//! and comes back in from the left. The columns are fixed rather than laid out per row so that a
//! run changing phase does not make every line in front of the reader jump.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use console::{Style, Term};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
use rorolala_cli_setups::ResProgressSetting;
use rorolala_utils_progress::{Direction, Progress, Signal, Transmitter};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use unicode_width::UnicodeWidthChar as _;

/// How many signals may be waiting to be drawn before the ones that do not fit are dropped.
///
/// Progress that has been overtaken is worth less than the work it describes, and a reader
/// this far behind has already been overtaken: the depth is what keeps a slow terminal from
/// becoming a slow run.
const DEPTH: usize = 256;

/// The characters a line is drawn with: what is behind the work, where the work is, and what is
/// ahead of it.
const CHARS: &str = "━╸─";

/// How much of the name of a piece being moved is shown, in columns.
///
/// A piece is named by the key its content is kept under, and a whole key is sixty-four
/// characters: the head is enough to tell one piece from the next, which is all the last column
/// is for.
const PIECE: usize = 7;

/// How wide the label column is: enough for `DOWNLOADING` and no wider.
const LABEL: usize = 12;

/// How much room the line keeps at the left and the right of the terminal.
const PAD_LEFT: usize = 4;
const PAD_RIGHT: usize = 2;

/// How long the waiting marquee takes to cross the line and come back in from the left.
const SNAKE_PERIOD_MS: u128 = 1_200;

/// The frames of a spinner for work that has no length to show.
const WAIT_TICKS: &str = "◜◝◞◟✔";

/// The frames of a spinner for work that knows its length but moves no data.
const WORK_TICKS: &str = "◜◝◞◟";

/// The fixed part of a row, which is what the marquee's length is worked out from.
const FIXED_WIDTH: usize = PAD_LEFT + 1 + 1 + LABEL + 1 + 6 + PIECE + PAD_RIGHT;

/// How long a run goes unwatched before it is watched.
///
/// Long enough that a run which is going to be over quickly is over before the rows would
/// have been shown, and short enough that a run which is not is watched almost from the start.
const DELAY: Duration = Duration::from_secs(2);

/// How often the clock is looked at while a run is still unwatched.
///
/// The reader is waiting on the clock rather than on the run, so it has to wake up for it:
/// this is how closely it keeps to [`DELAY`], and it is the only polling anything here does.
const LOOK: Duration = Duration::from_millis(10);

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
    /// Starts saying a run's progress, letting the bars out after the ordinary wait.
    #[must_use]
    pub fn start(setting: ResProgressSetting) -> Self {
        Self::after(setting, DELAY)
    }

    /// Starts saying a run's progress, letting the bars out after `delay`.
    ///
    /// A run is asked for a wait of its own when what it does first is the slow part: a recording
    /// reads the whole of every file it is to record before it writes anything, and a reader watching
    /// for that wants the bar sooner than the ordinary wait allows, a bar that appeared only once the
    /// reading was over would be a bar for nothing.
    #[must_use]
    pub fn after(setting: ResProgressSetting, delay: Duration) -> Self {
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
            _ => Watcher::Terminal(Terminal::new(delay)),
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
    /// The way to say anything is let go of before the reader is waited for, because that
    /// is what the reader is waiting for: a run that held on to it would be waiting on a thread
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
    /// Rows on the terminal, a task at a time.
    Terminal(Terminal),
    /// One record per signal, for another program to read.
    Records,
}

impl Watcher {
    /// Reads `receiver` until the run has nothing more to say.
    ///
    /// The reader is a thread of its own and not a task, so it waits the way a thread waits.
    /// What a run that is over quickly is shown is nothing at all: see [`Terminal::watch`].
    /// Records are not held back that way, a program reading them is reading what happened,
    /// and a run being over quickly does not make what happened less worth having.
    fn watch(&mut self, receiver: &mut mpsc::Receiver<Signal>) {
        match self {
            Self::Terminal(terminal) => terminal.watch(receiver),
            Self::Records => {
                while let Some(signal) = receiver.blocking_recv() {
                    Self::write_record(&signal);
                }
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

/// The rows a run is drawn as, stacked one under another.
struct Terminal {
    /// The rows, kept together so that they share the terminal and are redrawn as one block.
    bars: MultiProgress,
    /// What each row is, by the id the task is named by.
    drawn: BTreeMap<u64, ProgressBar>,
    /// How long a run goes unwatched before the rows are let out.
    delay: Duration,
}

impl Terminal {
    /// A terminal with nothing drawn on it yet, holding the rows back for `delay`.
    fn new(delay: Duration) -> Self {
        Self {
            bars: MultiProgress::with_draw_target(ProgressDrawTarget::hidden()),
            drawn: BTreeMap::new(),
            delay,
        }
    }

    /// Reads `receiver`, letting the rows out only once the run has gone on long enough to be
    /// worth watching.
    ///
    /// A run that is over in a moment has nothing to be watched for: rows that appear and are
    /// wiped again a moment later are a flicker where there was nothing to say, and a person
    /// reading them learns nothing they did not already know from the run having ended. So for
    /// the first little while nothing is let out, though what the run says is read all the
    /// same, the rows are as far along as the run is by the time they are shown, and a run
    /// that ends inside that while is never shown at all.
    ///
    /// The waiting is the clock's and not the run's, so it is done by looking rather than by
    /// blocking on something that may not come: a run can be silent for as long as it likes
    /// and still be shown on time.
    fn watch(&mut self, receiver: &mut mpsc::Receiver<Signal>) {
        let started = Instant::now();

        while started.elapsed() < self.delay {
            match receiver.try_recv() {
                Ok(signal) => self.take(&signal),
                Err(TryRecvError::Empty) => std::thread::sleep(LOOK),
                Err(TryRecvError::Disconnected) => return,
            }
        }

        // The rows have been drawn all along, into nothing: letting them out now shows them as
        // they are, and drawing them again shows them at once rather than at the next thing the
        // run says, which may be a long way off, since this is the run's own pace.
        self.bars.set_draw_target(ProgressDrawTarget::stderr());
        for bar in self.drawn.values() {
            bar.force_draw();
        }

        while let Some(signal) = receiver.blocking_recv() {
            self.take(&signal);
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

    /// Starts drawing a task: a row of its own when it is one, and a name on the row above it
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
            // The whole of the run: a row every other row is stacked under.
            (None, _) => {
                let bar = self.bars.add(bar(total, direction));
                bar.set_prefix(label_for(what, direction, total));

                if direction.is_some() && !what.is_empty() {
                    bar.set_message(fit(what, PIECE));
                }

                self.drawn.insert(id, bar);
            }
            // One direction of it: a row of its own, saying which way its work moves.
            (Some(_), Some(direction)) => {
                let bar = self.bars.add(bar(total, Some(direction)));
                bar.set_prefix(label_for("", Some(direction), total));

                if !what.is_empty() {
                    bar.set_message(fit(what, PIECE));
                }

                self.drawn.insert(id, bar);
            }
            // A piece being done right now: not a row of its own, but what the row above it is
            // moving, which is the last column of that row.
            (Some(parent), None) => {
                if let Some(above) = self.drawn.get(&parent) {
                    above.set_message(fit(what, PIECE));
                }
            }
        }
    }

    /// Stops drawing a task, and takes its row off the terminal.
    ///
    /// A task that is over has nothing left to say, so what is left behind is the whole of the
    /// run rather than a row of bars that have all stopped: a piece being done is not a row at
    /// all, so finishing one says nothing to the terminal and waits for the next name.
    fn finish(&mut self, id: u64) {
        if let Some(bar) = self.drawn.remove(&id) {
            bar.finish_and_clear();
        }
    }
}

/// What a row calls itself: the work's own word when it has one, and what the row is doing
/// otherwise.
///
/// The answer is padded or cut to [`LABEL`] columns so that the line after it starts in the
/// same place on every row, which is what keeps a run changing phase from moving the bar under
/// the reader's eye.
fn label_for(what: &str, direction: Option<Direction>, total: Option<u64>) -> String {
    let said = match (direction, total) {
        (Some(Direction::Up), _) => "UPLOADING",
        (Some(Direction::Down), _) => "DOWNLOADING",
        (None, None) => "WAITING",
        (None, Some(_)) if what.is_empty() => "WORKING",
        _ => what,
    };

    fit(said, LABEL)
}

/// Pads or cuts `text` to exactly `width` terminal columns.
///
/// A column is a terminal cell rather than a character: a Chinese word in a label is two cells
/// wide, and counting it as one would make the column after it stand in a different place in
/// each language.
fn fit(text: &str, width: usize) -> String {
    let mut shown = String::new();
    let mut used = 0;

    for character in text.chars() {
        let cells = character.width().unwrap_or(0);

        if used + cells > width {
            break;
        }

        shown.push(character);
        used += cells;
    }

    shown.push_str(&" ".repeat(width - used));

    shown
}

/// A row `total` long, drawn for work moving `direction`.
fn bar(total: Option<u64>, direction: Option<Direction>) -> ProgressBar {
    let bar = ProgressBar::new(total.unwrap_or(0));
    let style = match total {
        Some(_) => transfer_style(direction),
        None => wait_style(),
    };

    bar.set_style(style);

    bar
}

/// The row a run is drawn as while it has no length to show: a marquee crossing the line.
fn wait_style() -> ProgressStyle {
    let left = " ".repeat(PAD_LEFT);
    let right = " ".repeat(PAD_RIGHT);
    let width = snake_width();
    let block = (width / 4).max(4).min(width);
    let filled = Style::new().bright().yellow().for_stderr();
    let empty = Style::new().yellow().dim().for_stderr();

    // UNWRAP: this template is the program's own, so one that will not parse is a mistake in
    // the program rather than anything a run can bring about.
    ProgressStyle::with_template(&format!(
        "{left}{{spinner:.bright.yellow}} {{prefix:.bold}} {{snake}}   -- {{msg:.dim}}{right}"
    ))
    .unwrap()
    .with_key(
        "snake",
        move |state: &ProgressState, out: &mut dyn std::fmt::Write| {
            let elapsed = state.elapsed().as_millis();
            let cells = u128::try_from(width).unwrap_or(u128::MAX);
            let offset = (elapsed % SNAKE_PERIOD_MS) * cells / SNAKE_PERIOD_MS;
            let offset = usize::try_from(offset).unwrap_or(0) % width;
            let is_filled = |index: usize| (index + width - offset) % width < block;

            let mut line = String::with_capacity(width * 16);
            let mut index = 0;
            while index < width {
                let cell = is_filled(index);
                let start = index;
                while index < width && is_filled(index) == cell {
                    index += 1;
                }
                let segment = if cell { "━" } else { "─" }.repeat(index - start);
                let styled = if cell {
                    filled.apply_to(segment)
                } else {
                    empty.apply_to(segment)
                };
                let _ = write!(line, "{styled}");
            }
            let _ = out.write_str(&line);
        },
    )
    .tick_chars(WAIT_TICKS)
}

/// The row a transfer is drawn as: a line whose fill is the share of the work that is done.
fn transfer_style(direction: Option<Direction>) -> ProgressStyle {
    let (color, ticks) = match direction {
        Some(Direction::Up) => ("cyan", "↑↑"),
        Some(Direction::Down) => ("green", "↓↓"),
        None => ("blue", WORK_TICKS),
    };
    let left = " ".repeat(PAD_LEFT);
    let right = " ".repeat(PAD_RIGHT);

    // UNWRAP: this template is the program's own, so one that will not parse is a mistake in
    // the program rather than anything a run can bring about.
    ProgressStyle::with_template(&format!(
        "{left}{{spinner:.bright.{color}}} {{prefix:.bold}} \
         {{wide_bar:.bright.{color}/dim}} {{percent:>3}}% {{msg:.dim}}{right}"
    ))
    .unwrap()
    .progress_chars(CHARS)
    .tick_chars(ticks)
}

/// The width the marquee has, which is the terminal's width less every fixed column.
fn snake_width() -> usize {
    let columns = Term::stderr().size().1 as usize;
    let columns = if columns == 0 { 80 } else { columns };

    columns.saturating_sub(FIXED_WIDTH).max(4)
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::{Arc, Mutex};

    use indicatif::{ProgressBar, ProgressDrawTarget, TermLike};

    use super::{Direction, fit, label_for, transfer_style, wait_style};

    /// A terminal that keeps what is drawn on it, so a row can be read as text.
    #[derive(Debug, Clone, Default)]
    struct Captured(Arc<Mutex<String>>);

    impl TermLike for Captured {
        fn width(&self) -> u16 {
            100
        }

        fn move_cursor_up(&self, _: usize) -> io::Result<()> {
            Ok(())
        }

        fn move_cursor_down(&self, _: usize) -> io::Result<()> {
            Ok(())
        }

        fn move_cursor_right(&self, _: usize) -> io::Result<()> {
            Ok(())
        }

        fn move_cursor_left(&self, _: usize) -> io::Result<()> {
            Ok(())
        }

        fn write_line(&self, said: &str) -> io::Result<()> {
            self.0.lock().expect("the captured line").push_str(said);
            self.0.lock().expect("the captured line").push('\n');

            Ok(())
        }

        fn write_str(&self, said: &str) -> io::Result<()> {
            self.0.lock().expect("the captured row").push_str(said);

            Ok(())
        }

        fn clear_line(&self) -> io::Result<()> {
            Ok(())
        }

        fn flush(&self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn the_row_templates_are_this_programs_own() {
        let _ = wait_style();

        for direction in [None, Some(Direction::Up), Some(Direction::Down)] {
            let _ = transfer_style(direction);
        }
    }

    #[test]
    fn a_transfer_row_is_a_line_with_its_label_and_share() {
        let captured = Captured::default();
        let bar = ProgressBar::with_draw_target(
            Some(100),
            ProgressDrawTarget::term_like(Box::new(captured.clone())),
        );
        bar.set_style(transfer_style(Some(Direction::Up)));
        bar.set_prefix(label_for("", Some(Direction::Up), Some(100)));
        bar.set_message(fit("abcdefghi", 7));
        bar.set_position(50);
        bar.tick();

        let row = captured.0.lock().expect("the captured row").clone();

        assert!(row.contains("UPLOADING"));
        assert!(row.contains("50%"));
        assert!(row.contains('━'));
    }

    #[test]
    fn a_waiting_row_has_no_share_and_shows_the_marquee() {
        let captured = Captured::default();
        let bar = ProgressBar::with_draw_target(
            None,
            ProgressDrawTarget::term_like(Box::new(captured.clone())),
        );
        bar.set_style(wait_style());
        bar.set_prefix(label_for("", None, None));
        bar.tick();

        let row = captured.0.lock().expect("the captured row").clone();

        assert!(row.contains("WAITING"));
        assert!(row.contains("--"));
        assert!(row.contains('━') || row.contains('─'));
    }

    #[test]
    fn a_piece_is_shown_by_its_head_and_the_rest_is_left_out() {
        assert_eq!(
            fit(
                "5b4ac19152456173022e06a1614665a594c615ce1307d40f9a3d5006622991e4",
                7
            ),
            "5b4ac19"
        );
    }

    #[test]
    fn a_name_that_is_shorter_than_its_column_is_padded() {
        assert_eq!(fit("a key", 7), "a key  ");
        assert_eq!(fit("", 7), "       ");
    }

    #[test]
    fn a_label_is_counted_in_terminal_columns_rather_than_characters() {
        assert_eq!(fit("中文", 6), "中文  ");
        assert_eq!(fit("中文中文", 5), "中文 ");
    }
}
