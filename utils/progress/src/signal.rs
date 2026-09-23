//! What a run says about itself, and which way its work moves.

use serde::Serialize;

/// Which way a piece of work moves.
///
/// Only a transfer moves a way: what is said about work that moves no data carries none, and
/// how its line is drawn is then only about how much of it is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Work leaving this side.
    Up,
    /// Work arriving at this side.
    Down,
}

/// One thing a run says about itself while it runs.
///
/// A signal names the work it is about by an id, so what is said about one task cannot be
/// taken for what is said about another. Nothing here knows how a signal is drawn: a reader
/// may draw it, write it down as a record, or drop it, and the run goes on either way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "signal", rename_all = "snake_case")]
pub enum Signal {
    /// A task has begun, and is `total` long when that is known already.
    Begin {
        /// What the task is named by, and what its later signals are about.
        id: u64,
        /// The task this one is part of, when it is part of another one.
        parent: Option<u64>,
        /// What the task is called.
        what: String,
        /// Which way its work moves, when it moves any.
        direction: Option<Direction>,
        /// How much of the task there is in all, when that is known from the start.
        total: Option<u64>,
    },
    /// A task has moved, and `done` of it is behind it.
    Advance {
        /// The task that moved.
        id: u64,
        /// How much of the task is done, counted the way its `total` is.
        done: u64,
    },
    /// A task is over, whether it ran out or was given up on.
    Finish {
        /// The task that is over.
        id: u64,
    },
}
