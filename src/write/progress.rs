//! What a write says about itself while it runs, and the handle a C caller polls it by.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_progress::{Direction, Signal, Signals};

/// A write's progress, collected until the caller asks for it.
///
/// The write runs on the caller's thread, and a C# caller runs it on a worker; the queue is the one
/// thing between them. A queue that grows without bound would be a leak for a run nobody watches, so
/// it keeps only the newest events: progress that has been overtaken is worth less than the memory
/// it would hold.
///
/// The queue is shared rather than owned, so a handle can be handed to the write as a sink and kept
/// by the caller that polls it: the C ABI's pointer is a borrow of the one owner, and the clone the
/// write takes is what it says its events to.
#[lazyffi(export = RolaProgress)]
#[derive(Debug, Clone)]
pub struct WriteProgress {
    /// The events not yet read, oldest first.
    queue: Arc<Mutex<VecDeque<Signal>>>,
}

impl WriteProgress {
    /// A progress handle with nothing said yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// The next event, or nothing when nothing is waiting.
    #[must_use]
    pub fn next_event(&self) -> Option<Signal> {
        let Ok(mut queue) = self.queue.lock() else {
            return None;
        };

        queue.pop_front()
    }
}

impl Default for WriteProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl Signals for WriteProgress {
    fn signal(&self, signal: Signal) {
        let Ok(mut queue) = self.queue.lock() else {
            return;
        };

        // A reader that stopped reading is a reader that stopped watching; the work goes on and the
        // events it did not see are gone.
        if queue.len() >= 256 {
            let _ = queue.pop_front();
        }
        queue.push_back(signal);
    }
}

/// Makes a progress handle for a C caller.
///
/// # FFI
///
/// A `RolaProgress *` the caller releases with `free_rola_progress`, or null when one cannot be
/// made.
#[must_use]
#[lazyffi(export = rola_progress_new)]
pub fn new_progress() -> Option<WriteProgress> {
    Some(WriteProgress::new())
}

/// The progress a write says its events to, from the handle a C caller holds.
///
/// The handle shares its queue, so the sink the write is given and the handle the caller polls are
/// one queue: what the write says after the call is still there to be read.
#[must_use]
pub fn watching(progress: &WriteProgress) -> rorolala_utils_progress::Progress {
    rorolala_utils_progress::Progress::to(progress.clone())
}

/// One thing a write has said about itself, as the C ABI hands it over.
///
/// A transparent enum has no pointer of its own, so "nothing is waiting" cannot be a null of the
/// enum itself; it is a null of this handle instead, which is what a poll answers with. What the
/// write said is read from the handle by [`signal`](Self::signal).
///
/// # FFI
///
/// A `RolaProgressEvent *` the caller releases with `free_rola_progress_event`, or null when nothing
/// is waiting. What it carries is a `RolaProgressSignal` by value, whose `char *` fields the caller
/// releases with `free_string`.
#[lazyffi(export = RolaProgressEvent)]
#[derive(Debug, Clone)]
pub struct ProgressEvent {
    /// What the write said.
    signal: ProgressSignal,
}

impl ProgressEvent {
    /// The event `signal` says, with the numbers that may be absent said apart from them.
    fn taken(signal: Signal) -> Self {
        Self {
            signal: ProgressSignal::from_signal(signal),
        }
    }
}

#[lazyffi(export = rola_)]
impl ProgressEvent {
    /// What the write said.
    #[must_use]
    pub fn signal(&self) -> ProgressSignal {
        self.signal.clone()
    }
}

/// One thing a write has said about itself, as the C ABI reads it.
///
/// It mirrors [`Signal`], with the numbers that may be absent said as a presence flag beside them:
/// a scalar has no null to spend at the boundary, so a `Begin` that is part of no other task has
/// `parent_present` false and a `parent` that says nothing, and one whose length is not known has
/// `total_present` false in the same way. A task that moves no data carries an empty `direction`.
///
/// # FFI
///
/// A tagged union: `RolaProgressSignalTag` says which event it is, and `RolaProgressSignalPayload`
/// holds its fields. Every `char *` is owned by the caller and released with `free_string`.
#[lazyffi(export = RolaProgressSignal)]
#[derive(Debug, Clone)]
pub enum ProgressSignal {
    /// A task has begun.
    Begin {
        /// What the task is named by.
        id: u64,
        /// Whether it is part of another task.
        parent_present: bool,
        /// The task it is part of, or 0 when it is part of none.
        parent: u64,
        /// What the task is called.
        what: String,
        /// Which way its work moves, `up` or `down`, or empty when it moves no data.
        direction: String,
        /// Whether how much of the task there is in all is known.
        total_present: bool,
        /// How much of the task there is in all, or 0 when it is not known.
        total: u64,
    },
    /// A task has moved.
    Advance {
        /// The task that moved.
        id: u64,
        /// How much of the task is done.
        done: u64,
    },
    /// A task is over.
    Finish {
        /// The task that is over.
        id: u64,
    },
}

impl ProgressSignal {
    /// The signal `signal` says, with the numbers that may be absent said apart from them.
    fn from_signal(signal: Signal) -> Self {
        match signal {
            Signal::Begin {
                id,
                parent,
                what,
                direction,
                total,
            } => Self::Begin {
                id,
                parent_present: parent.is_some(),
                parent: parent.unwrap_or_default(),
                what,
                direction: match direction {
                    Some(Direction::Up) => "up",
                    Some(Direction::Down) => "down",
                    None => "",
                }
                .to_owned(),
                total_present: total.is_some(),
                total: total.unwrap_or_default(),
            },
            Signal::Advance { id, done } => Self::Advance { id, done },
            Signal::Finish { id } => Self::Finish { id },
        }
    }
}

/// The next event a write has said.
///
/// # FFI
///
/// A `RolaProgressEvent *` the caller releases with `free_rola_progress_event`, or null when nothing
/// is waiting. What it carries is read with `rola_progress_event_signal`, a `RolaProgressSignal` by
/// value whose `char *` fields the caller releases with `free_string`.
#[must_use]
#[lazyffi(export = rola_progress_next)]
pub fn next_progress(progress: &WriteProgress) -> Option<ProgressEvent> {
    progress.next_event().map(ProgressEvent::taken)
}

#[cfg(test)]
mod tests {
    use rorolala_utils_progress::{Direction, Signal, Signals as _};

    use super::{
        ProgressEvent, ProgressSignal, WriteProgress, new_progress, next_progress, watching,
    };

    fn begin(direction: Option<Direction>, total: Option<u64>) -> Signal {
        Signal::Begin {
            id: 7,
            parent: None,
            what: "track".to_owned(),
            direction,
            total,
        }
    }

    fn answered(signal: Signal) -> ProgressEvent {
        let progress = WriteProgress::new();
        progress.signal(signal);

        next_progress(&progress).unwrap()
    }

    #[test]
    fn a_new_handle_has_nothing_waiting() {
        let progress = WriteProgress::new();

        assert!(progress.next_event().is_none());
        assert!(next_progress(&progress).is_none());
        assert!(new_progress().is_some());
    }

    #[test]
    fn an_event_is_read_once_in_the_order_it_was_said() {
        let progress = WriteProgress::new();
        progress.signal(Signal::Advance { id: 1, done: 3 });
        progress.signal(Signal::Finish { id: 1 });

        assert_eq!(
            progress.next_event(),
            Some(Signal::Advance { id: 1, done: 3 })
        );
        assert_eq!(progress.next_event(), Some(Signal::Finish { id: 1 }));
        assert!(progress.next_event().is_none());
    }

    #[test]
    fn a_begin_is_handed_over_with_absent_numbers_said_apart() {
        let ProgressSignal::Begin {
            id,
            parent_present,
            parent,
            what,
            direction,
            total_present,
            total,
        } = answered(begin(Some(Direction::Up), Some(10))).signal()
        else {
            panic!("a begin was said");
        };

        assert_eq!(id, 7);
        assert!(!parent_present);
        assert_eq!(parent, 0);
        assert_eq!(what, "track");
        assert_eq!(direction, "up");
        assert!(total_present);
        assert_eq!(total, 10);
    }

    #[test]
    fn a_begin_without_a_direction_or_a_total_says_so() {
        let ProgressSignal::Begin {
            direction,
            total_present,
            total,
            ..
        } = answered(begin(None, None)).signal()
        else {
            panic!("a begin was said");
        };

        assert_eq!(direction, "");
        assert!(!total_present);
        assert_eq!(total, 0);
    }

    #[test]
    fn a_begin_says_which_way_its_work_moves() {
        let ProgressSignal::Begin { direction, .. } =
            answered(begin(Some(Direction::Down), Some(1))).signal()
        else {
            panic!("a begin was said");
        };

        assert_eq!(direction, "down");
    }

    #[test]
    fn advance_and_finish_keep_the_numbers_they_were_said_with() {
        assert!(matches!(
            answered(Signal::Advance { id: 2, done: 9 }).signal(),
            ProgressSignal::Advance { id: 2, done: 9 }
        ));
        assert!(matches!(
            answered(Signal::Finish { id: 2 }).signal(),
            ProgressSignal::Finish { id: 2 }
        ));
    }

    #[test]
    fn the_queue_keeps_only_the_newest_events() {
        let progress = WriteProgress::new();

        for id in 0..300 {
            progress.signal(Signal::Finish { id });
        }

        // Three hundred were said and the queue holds 256, so the oldest 44 are gone.
        for id in 44..300 {
            assert_eq!(progress.next_event(), Some(Signal::Finish { id }));
        }
        assert!(progress.next_event().is_none());
    }

    #[test]
    fn a_watched_handle_and_the_caller_share_one_queue() {
        let progress = WriteProgress::new();

        let watched = watching(&progress);

        let mut task = watched.begin("work", Some(1));
        task.advance(1);

        assert!(matches!(
            progress.next_event(),
            Some(Signal::Begin { what, .. }) if what == "work"
        ));
        assert!(matches!(
            progress.next_event(),
            Some(Signal::Advance { done: 1, .. })
        ));
    }
}
