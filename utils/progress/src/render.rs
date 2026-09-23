//! The lines what was said is drawn as.
//!
//! Nothing here writes to anything: these are the shapes a reader turns a run into, so the
//! same run can be a bar in a terminal and a record in a file without being run twice.

use crate::signal::Direction;

/// How wide the bar of a whole run is, in cells.
pub const TOTAL_WIDTH: usize = 20;

/// How wide the bar of one task is, in cells.
pub const TASK_WIDTH: usize = 10;

/// The frames a spinner turns through, in the order it turns.
const FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// The arrow a task's line carries when its work leaves this side.
const UPLOAD: char = '↑';

/// The arrow a task's line carries when its work arrives at this side.
const DOWNLOAD: char = '↓';

/// The mark a bar carries where its work is up to, when that work leaves this side.
const UPLOAD_MARK: char = '>';

/// The mark a bar carries where its work is up to, when that work arrives at this side.
const DOWNLOAD_MARK: char = '<';

/// The frame a spinner that has turned `tick` times shows.
///
/// A spinner is for work whose length is not known, so it says only that something is
/// happening: it turns through its frames and comes round, which is what makes a still
/// terminal look like a live one.
#[must_use]
pub fn spinner_frame(tick: usize) -> char {
    FRAMES[tick % FRAMES.len()]
}

/// The bar of `fraction` over `width` cells, as `[====    ]`.
///
/// A fraction outside `0` to `1` is taken to be an end of it, and one that is not a number at
/// all is taken to be none of it, so a bar is always the width it was asked for and a reader
/// is never shown a fraction it cannot make sense of.
#[must_use]
pub fn bar(fraction: f64, width: usize) -> String {
    let filled = cells(fraction, width);

    format!("[{}{}]", "=".repeat(filled), " ".repeat(width - filled))
}

/// The line of a whole run, which is a bar with nothing to count inside it.
#[must_use]
pub fn total_line(spinner: char, fraction: f64) -> String {
    format!("{spinner} {}", bar(fraction, TOTAL_WIDTH))
}

/// The line of one task of a run, which says the way its work moves and what it is doing.
///
/// A task with no name of its own is drawn without one — the arrow and the bar are the whole
/// of what it has to say — which is what a task named by what it does rather than by a word
/// wants.
///
/// `sub` names the piece of the task that is being done right now, when there is one: a task
/// that moves a store key by key is one line the whole way, and the name at its end is what
/// changes as it goes.
#[must_use]
pub fn task_line(
    spinner: char,
    name: &str,
    direction: Direction,
    fraction: f64,
    sub: &str,
) -> String {
    let arrow = match direction {
        Direction::Up => UPLOAD,
        Direction::Down => DOWNLOAD,
    };
    let bar = directed_bar(fraction, direction, TASK_WIDTH);
    let line = if name.is_empty() {
        format!("{spinner} {arrow} {bar}")
    } else {
        format!("{spinner} {name} {arrow} {bar}")
    };

    if sub.is_empty() {
        line
    } else {
        format!("{line} {sub}")
    }
}

/// How much of a whole is done, from `0` to `1`.
///
/// A whole that is not known, or that is nothing, has nothing done of it: there is no
/// fraction of no work, and saying there is would draw a bar that never moves rather than one
/// that is honestly unknown.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn fraction(done: u64, total: Option<u64>) -> f64 {
    match total {
        Some(total) if total > 0 => done as f64 / total as f64,
        _ => 0.0,
    }
}

/// The bar of a task, whose mark sits where its work has reached.
///
/// The mark is what tells the two ways apart when a bar is read on its own: leaving this side
/// fills from the left, arriving at it from the right.
fn directed_bar(fraction: f64, direction: Direction, width: usize) -> String {
    let filled = cells(fraction, width);

    // Nothing is done, so there is nowhere for a mark to sit that is not a lie about how far
    // the work has got.
    if filled == 0 {
        return format!("[{}]", " ".repeat(width));
    }

    let behind = "=".repeat(filled - 1);
    let ahead = " ".repeat(width - filled);

    match direction {
        Direction::Up => format!("[{behind}{UPLOAD_MARK}{ahead}]"),
        Direction::Down => format!("[{ahead}{DOWNLOAD_MARK}{behind}]"),
    }
}

/// How many of `width` cells `fraction` fills.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn cells(fraction: f64, width: usize) -> usize {
    if !fraction.is_finite() {
        return 0;
    }

    let filled = (fraction.clamp(0.0, 1.0) * width as f64).round();

    // The clamp above keeps this in range; the cast is the round trip back to cells.
    filled.max(0.0) as usize
}

#[cfg(test)]
mod tests {
    use super::{bar, fraction, spinner_frame, task_line, total_line};
    use crate::signal::Direction;

    #[test]
    fn a_bar_fills_by_whole_cells_and_holds_its_width() {
        assert_eq!(bar(0.0, 20), "[                    ]");
        assert_eq!(bar(0.6, 20), "[============        ]");
        assert_eq!(bar(1.0, 20), "[====================]");

        // A fraction past the end is still the end of the bar, and one that is not a number
        // at all is none of it.
        assert_eq!(bar(2.0, 20), "[====================]");
        assert_eq!(bar(f64::NAN, 20), "[                    ]");
    }

    #[test]
    fn the_whole_of_a_run_is_a_bar_after_a_turning_spinner() {
        assert_eq!(total_line('⠋', 0.6), "⠋ [============        ]");
    }

    #[test]
    fn a_task_is_drawn_the_way_its_work_moves() {
        assert_eq!(
            task_line('⠋', "Task", Direction::Up, 0.4, "Sub"),
            "⠋ Task ↑ [===>      ] Sub"
        );
        assert_eq!(
            task_line('⠋', "Task", Direction::Down, 0.4, "Sub"),
            "⠋ Task ↓ [      <===] Sub"
        );

        // A task with nothing named inside it ends at its bar, and one with nothing done has
        // no mark for the work to be at.
        assert_eq!(
            task_line('⠋', "Task", Direction::Up, 1.0, ""),
            "⠋ Task ↑ [=========>]"
        );
        assert_eq!(
            task_line('⠋', "Task", Direction::Up, 0.0, "Sub"),
            "⠋ Task ↑ [          ] Sub"
        );

        // A task with no name of its own is the arrow and the bar alone.
        assert_eq!(
            task_line('⠋', "", Direction::Up, 0.4, "a key"),
            "⠋ ↑ [===>      ] a key"
        );
    }

    #[test]
    fn the_spinner_turns_through_its_frames_and_comes_round() {
        assert_ne!(spinner_frame(0), spinner_frame(1));
        assert_eq!(spinner_frame(0), spinner_frame(10));
    }

    #[test]
    fn nothing_is_done_of_a_whole_that_is_not_known_or_is_nothing() {
        assert!((fraction(5, Some(10)) - 0.5).abs() < f64::EPSILON);
        assert!(fraction(5, None).abs() < f64::EPSILON);
        assert!(fraction(5, Some(0)).abs() < f64::EPSILON);
    }
}
