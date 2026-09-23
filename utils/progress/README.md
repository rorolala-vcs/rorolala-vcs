# Rorolala Utils Progress

What a long run says about itself while it runs.

A crate of one half: the **saying**. A [`Progress`] is a handle a work hands down to what it
starts, and a [`Task`] is one thing being done — begun, moved, and ended. What is said goes
down a [`Transmitter`] and out of the crate entirely, so nothing here knows or decides how a
terminal is drawn on: a reader may draw bars, write the signals out as records, or ignore
them, and a run that wants no progress at all says nothing with [`Progress::silent`].

```rust
use rorolala_utils_progress::{Direction, Progress, Transmitter};

// A run that is watched: the reader at the other end is someone else's, and what becomes of
// what is said there is theirs to decide.
let (transmitter, mut receiver) = Transmitter::channel(16);
let progress = Progress::to(transmitter);

// The whole of what is being done, and the one direction of it that is being done now.
let mut everything = progress.begin("sync", Some(2));
let mut carrying = everything.spawn(Direction::Up, "", Some(1));
carrying.advance_by(1);
carrying.finish();
everything.advance_by(1);

// What was said is waiting to be read, and nothing had to wait for it to be read.
assert!(receiver.try_recv().is_ok());
```

A signal names the work it is about by an id and, where it is part of another task, the id of
that one — so a task that works through many things is one line the whole way, with the name
of the thing being worked on changing as it goes. Saying is never allowed to hold up the work:
a reader that has fallen behind is left behind.
