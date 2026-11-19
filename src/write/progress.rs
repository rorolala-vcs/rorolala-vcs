//! What a write says about itself while it runs, and the handle a C caller polls it by.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_progress::{Signal, Signals};

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
    queue: Arc<Mutex<VecDeque<String>>>,
}

impl WriteProgress {
    /// A progress handle with nothing said yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// The next event as JSON, or the JSON `null` when nothing is waiting.
    #[must_use]
    pub fn next_json(&self) -> String {
        let Ok(mut queue) = self.queue.lock() else {
            return "null".to_owned();
        };

        queue.pop_front().unwrap_or_else(|| "null".to_owned())
    }
}

impl Default for WriteProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl Signals for WriteProgress {
    fn signal(&self, signal: Signal) {
        let Ok(text) = serde_json::to_string(&signal) else {
            return;
        };

        let Ok(mut queue) = self.queue.lock() else {
            return;
        };

        // A reader that stopped reading is a reader that stopped watching; the work goes on and the
        // events it did not see are gone.
        if queue.len() >= 256 {
            let _ = queue.pop_front();
        }
        queue.push_back(text);
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

/// The next event a write has said, as JSON.
///
/// # FFI
///
/// An owned string the caller releases with `free_string`, or the JSON `null` when nothing is
/// waiting. The shape is `rorolala_utils_progress::Signal`'s serde output.
#[must_use]
#[lazyffi(export = rola_progress_next_json)]
pub fn next_progress(progress: &WriteProgress) -> String {
    progress.next_json()
}
