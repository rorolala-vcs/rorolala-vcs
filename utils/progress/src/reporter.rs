//! Saying what is being done, to whatever is listening.
//!
//! The work is handed a [`Progress`] and says what it is doing to it. What that turns into is
//! not the work's business: [`Progress::silent`] says it to nobody, and the same work runs
//! with a bar, a record, or nothing at all.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::mpsc;

use crate::signal::{Direction, Signal};

/// What a run says its progress to.
///
/// A sink is told about work and decides what becomes of it: drawn, written down, or dropped.
/// Nothing about a sink may hold the work up — the work is the point and the progress is about
/// it, so a sink that cannot keep up is left behind rather than waited for.
pub trait Signals: Send + Sync {
    /// Takes one thing said about the work.
    fn signal(&self, signal: Signal);
}

/// A sink that keeps nothing, for a run that was asked to keep quiet.
#[derive(Debug, Clone, Copy, Default)]
pub struct Silent;

impl Signals for Silent {
    fn signal(&self, _: Signal) {}
}

/// The shared side of a run's progress: where what is said goes, and which id is next.
struct Hub {
    sink: Arc<dyn Signals>,
    next: AtomicU64,
}

impl Hub {
    /// Takes the next id, so that one task's signals can never be read as another's.
    fn take_id(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }
}

/// A run's progress, and the way to begin work that reports to it.
///
/// Cloning shares the ids, so one progress can be handed to every part of a run and what each
/// part says still names it apart from the rest.
#[derive(Clone)]
pub struct Progress {
    hub: Arc<Hub>,
}

impl Progress {
    /// Progress said to `sink`.
    #[must_use]
    pub fn to(sink: impl Signals + 'static) -> Self {
        Self {
            hub: Arc::new(Hub {
                sink: Arc::new(sink),
                next: AtomicU64::new(0),
            }),
        }
    }

    /// Progress nobody hears, for a run that wants nothing shown.
    #[must_use]
    pub fn silent() -> Self {
        Self::to(Silent)
    }

    /// Begins a task called `what`, which is `total` long when that is known already.
    ///
    /// The task is over when it is dropped, so a work that ends early — by returning, by a
    /// `?`, or by being given up on — ends the task it began rather than leaving a reader
    /// watching something that will never finish.
    #[must_use]
    pub fn begin(&self, what: impl Into<String>, total: Option<u64>) -> Task {
        begin_under(&self.hub, None, what.into(), None, total)
    }
}

/// One thing being done, and the way to say how far it has got.
///
/// A task says nothing until it is moved: a work that begins a task and goes straight to its
/// end is one line that closes as soon as it opens, which is what a reader sees for work that
/// was over before it could be watched.
pub struct Task {
    hub: Arc<Hub>,
    id: u64,
    total: Option<u64>,
    /// How far the task is, whether or not that has been said yet.
    done: u64,
    /// How far the task had got when it was last said, which is what keeps the chatter down.
    said: u64,
    over: bool,
}

impl Task {
    /// What this task is named by, so another can be begun as part of it.
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// Begins work that is part of this task, moving `direction` and `total` long.
    ///
    /// Being part of a task is what lets a reader show one line for the whole of it and name
    /// inside that line what is being done right now: a task that moves a store one key at a
    /// time is one line with many names.
    #[must_use]
    pub fn spawn(&self, direction: Direction, what: impl Into<String>, total: Option<u64>) -> Self {
        begin_under(
            &self.hub,
            Some(self.id),
            what.into(),
            Some(direction),
            total,
        )
    }

    /// Names the piece of this task being done right now.
    ///
    /// A task that works through many things — a store key by key — is one line the whole way,
    /// and this is what changes on it: the marker lasts as long as it is held, so a reader
    /// shows the thing being worked on and moves on when the work does. It moves nothing
    /// itself, and so carries no direction and no length.
    #[must_use]
    pub fn doing(&self, what: impl Into<String>) -> Self {
        begin_under(&self.hub, Some(self.id), what.into(), None, None)
    }

    /// Says the task is `done` of the way through, counted the way its `total` is.
    ///
    /// Saying how far a task is does not mean it is heard every time: what a reader cannot
    /// show is not worth the saying, so only a change it could draw is passed on.
    pub fn advance(&mut self, done: u64) {
        if self.over {
            return;
        }

        self.done = done;
        self.report();
    }

    /// Says the task has gone `by` further.
    pub fn advance_by(&mut self, by: u64) {
        if self.over {
            return;
        }

        self.done = self.done.saturating_add(by);
        self.report();
    }

    /// Says the task is over.
    ///
    /// Ending a task twice says it once: it is dropped as well as finished when a work ends
    /// it and then lets it go, and a reader told twice would draw it twice.
    pub fn finish(&mut self) {
        if self.over {
            return;
        }

        self.over = true;
        self.hub.sink.signal(Signal::Finish { id: self.id });
    }

    fn report(&mut self) {
        if !worth_saying(self.said, self.done, self.total) {
            return;
        }

        self.said = self.done;
        self.hub.sink.signal(Signal::Advance {
            id: self.id,
            done: self.done,
        });
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        self.finish();
    }
}

/// Begins a task under `hub`, saying so before it is handed back so that a reader hears of a
/// task before it hears anything of what that task does.
fn begin_under(
    hub: &Arc<Hub>,
    parent: Option<u64>,
    what: String,
    direction: Option<Direction>,
    total: Option<u64>,
) -> Task {
    let id = hub.take_id();
    hub.sink.signal(Signal::Begin {
        id,
        parent,
        what,
        direction,
        total,
    });

    Task {
        hub: Arc::clone(hub),
        id,
        total,
        done: 0,
        said: 0,
        over: false,
    }
}

/// Whether a reader could show `done` as anything other than what it was already showing.
///
/// A bar moves by whole cells, so a change smaller than one of them draws the same line twice:
/// saying it would be churn a reader cannot see, and the work would pay for it. The end is
/// always worth saying, since a bar that stops short of full reads as work that did not finish.
fn worth_saying(said: u64, done: u64, total: Option<u64>) -> bool {
    if done == said {
        return false;
    }

    // Work that went backwards is news whatever the bar can show, and so is any progress on
    // work whose length is not known: an open bar has no cells to be a fraction of.
    if done < said {
        return true;
    }

    let Some(total) = total.filter(|total| *total > 0) else {
        return true;
    };

    done * 100 / total != said * 100 / total
}

/// The end of a channel a run says its progress down.
///
/// The work and the reader run apart, so neither waits for the other: a reader may be a
/// terminal being redrawn, and a bar that held up the work it describes would make the work as
/// slow as watching it.
pub struct Transmitter {
    sender: mpsc::Sender<Signal>,
}

impl Transmitter {
    /// Makes a channel `capacity` signals deep, and the end a reader takes them from.
    ///
    /// The depth is the slack between a work and a reader: past it, what is said is dropped
    /// rather than queued, so a reader that has stalled costs the work nothing.
    #[must_use]
    pub fn channel(capacity: usize) -> (Self, mpsc::Receiver<Signal>) {
        let (sender, receiver) = mpsc::channel(capacity);

        (Self { sender }, receiver)
    }
}

impl Signals for Transmitter {
    fn signal(&self, signal: Signal) {
        // A reader that has fallen behind is left behind: what it missed is a step of a bar
        // that has already moved on, and waiting for it would make the work carry the reader
        // rather than the reader watching the work.
        let _ = self.sender.try_send(signal);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{Progress, Signals};
    use crate::signal::{Direction, Signal};

    /// A sink that keeps everything it is told, for a test to read back.
    #[derive(Clone, Default)]
    struct Recorded(Arc<Mutex<Vec<Signal>>>);

    impl Recorded {
        fn said(&self) -> Vec<Signal> {
            self.0.lock().expect("the record is not poisoned").clone()
        }

        fn count_finishes(&self) -> usize {
            self.said()
                .iter()
                .filter(|signal| matches!(signal, Signal::Finish { .. }))
                .count()
        }
    }

    impl Signals for Recorded {
        fn signal(&self, signal: Signal) {
            self.0
                .lock()
                .expect("the record is not poisoned")
                .push(signal);
        }
    }

    #[test]
    fn each_task_is_named_apart_and_says_it_has_begun() {
        let sink = Recorded::default();
        let progress = Progress::to(sink.clone());
        let first = progress.begin("first", Some(10));
        let second = progress.begin("second", None);

        assert_ne!(first.id(), second.id());

        let said = sink.said();
        assert!(matches!(said[0], Signal::Begin { id, parent: None, .. } if id == first.id()));
        assert!(matches!(said[1], Signal::Begin { id, parent: None, .. } if id == second.id()));
    }

    #[test]
    fn work_that_is_part_of_a_task_says_what_it_is_part_of() {
        let sink = Recorded::default();
        let progress = Progress::to(sink.clone());
        let whole = progress.begin("sync", Some(100));
        let part = whole.spawn(Direction::Up, "a key", Some(1000));

        let said = sink.said();
        assert!(matches!(
            said[1],
            Signal::Begin {
                id,
                parent: Some(parent),
                direction: Some(Direction::Up),
                total: Some(1000),
                ..
            } if id == part.id() && parent == whole.id()
        ));
    }

    #[test]
    fn a_task_that_is_dropped_says_it_is_over() {
        let sink = Recorded::default();
        let progress = Progress::to(sink.clone());

        {
            let _task = progress.begin("work", None);
        }

        assert_eq!(sink.count_finishes(), 1);
    }

    #[test]
    fn a_task_is_told_to_finish_once_however_often_it_is() {
        let sink = Recorded::default();
        let progress = Progress::to(sink.clone());
        let mut task = progress.begin("work", None);

        task.finish();
        task.finish();

        // Nothing is said of a task that is over, so a work that ends it and then moves it
        // does not bring it back.
        task.advance(1);

        assert_eq!(sink.count_finishes(), 1);
        assert_eq!(sink.said().len(), 2);
    }

    #[test]
    fn progress_a_reader_could_not_show_is_not_said() {
        let sink = Recorded::default();
        let progress = Progress::to(sink.clone());
        let mut task = progress.begin("work", Some(1000));

        // Not a whole percent of the bar yet: what a reader would draw is what it already
        // drew.
        task.advance(1);
        assert_eq!(sink.said().len(), 1);

        // A whole percent is a cell of the bar, so it is worth drawing.
        task.advance(10);
        assert_eq!(sink.said().len(), 2);

        // The end is said even when the bar would not have moved there.
        task.advance(1000);
        assert_eq!(sink.said().len(), 3);
    }

    #[test]
    fn all_progress_of_work_of_unknown_length_is_said() {
        let sink = Recorded::default();
        let progress = Progress::to(sink.clone());
        let mut task = progress.begin("work", None);

        task.advance(1);
        task.advance(2);

        assert_eq!(sink.said().len(), 3);
    }

    #[test]
    fn a_piece_being_done_is_named_under_the_task_it_is_part_of() {
        let sink = Recorded::default();
        let progress = Progress::to(sink.clone());
        let mut task = progress.begin("work", Some(2));

        for name in ["first", "second"] {
            let _piece = task.doing(name);
            task.advance_by(1);
        }

        let said = sink.said();
        assert!(matches!(
            said[1],
            Signal::Begin {
                parent: Some(parent),
                direction: None,
                total: None,
                ref what,
                ..
            } if parent == task.id() && what == "first"
        ));
        // The marker is dropped at the end of its turn, so the next piece is named after it.
        assert!(matches!(said[3], Signal::Finish { .. }));
        assert!(matches!(said[4], Signal::Begin { ref what, .. } if what == "second"));
        assert!(matches!(said[5], Signal::Advance { done: 2, .. }));
    }

    #[test]
    fn a_silent_run_says_nothing_and_runs_all_the_same() {
        let progress = Progress::silent();
        let mut task = progress.begin("work", Some(10));
        let mut part = task.spawn(Direction::Down, "a key", Some(10));

        task.advance(5);
        part.advance_by(5);
        part.finish();
    }
}
