//! The styles Rorolala's messages are drawn in.
//!
//! A message is not written here but somewhere in the program; what is here is the
//! handful of lines that come up again and again, written once so that they come out
//! the same wherever they are used. Each is written in the color language and drawn by
//! [`TextRendering`], so what a line looks like stays the theme's decision rather than
//! the caller's.
//!
//! Every line comes back without the newline that ends it, so that lines are joined by
//! the caller — `format!("{}\n{}", err_line(..), help_line(..))` — rather than arriving
//! with a blank line after each.

use crate::{TextRendering, ThemeChoice, theme_choice};

/// The line an error is reported on.
///
/// `prefix` is what the line is called — `ERROR`, in Rorolala's own messages — and
/// everything drawn around it is fixed: the `::` and the `=>` in the level's colour
/// where the glyphs are missing, and the mark, the background and the bold where they
/// are there.
///
/// The content is drawn in italics, and is written in the color language like anything
/// else, so a style or a character it names is drawn too.
///
/// ```rust
/// use rorolala_utils_cli_theme::{ThemeChoice, err_line, set_enabled, set_theme_choice};
///
/// set_enabled(false);
/// set_theme_choice(ThemeChoice::Simple);
/// assert_eq!(err_line("ERROR", "Fail to load vault!"), "::ERROR=> Fail to load vault!");
/// ```
#[must_use]
pub fn err_line(prefix: &str, content: &str) -> String {
    drawn(Level::Error, prefix, content)
}

/// The line a warning is reported on.
///
/// The line [`err_line()`] draws, in the colour a warning is drawn in.
#[must_use]
pub fn warn_line(prefix: &str, content: &str) -> String {
    drawn(Level::Warning, prefix, content)
}

/// The line a way out is offered on.
///
/// The line [`err_line()`] draws, in the colour help is drawn in.
#[must_use]
pub fn help_line(prefix: &str, content: &str) -> String {
    drawn(Level::Help, prefix, content)
}

/// The line an error is reported on, written out where it is used.
///
/// The message is a [`format!`] string and a color-language one at once, so a value is
/// put into it and a style is written around it. Written without a name for the line,
/// it is reported as an `ERROR`; the name is written before a `=>` where it is not:
///
/// ```rust
/// use rorolala_utils_cli_theme::{ThemeChoice, err_line, set_enabled, set_theme_choice};
///
/// set_enabled(false);
/// set_theme_choice(ThemeChoice::Simple);
/// assert_eq!(err_line!("Fail to load **{}**!", "vault"), "::ERROR=> Fail to load vault!");
///
/// const LOAD: &str = "LOAD";
/// assert_eq!(err_line!(LOAD => "Fail to load {}!", "vault"), "::LOAD=> Fail to load vault!");
/// ```
#[macro_export]
macro_rules! err_line {
    // The name the line is reported under, and what it says.
    ($prefix:expr => $($message:tt)+) => {
        $crate::err_line(&$prefix, &$crate::__message!($($message)+))
    };
    // What it says, under the name it is usually reported with.
    ($($message:tt)+) => {
        $crate::err_line("ERROR", &$crate::__message!($($message)+))
    };
}

/// The line a warning is reported on, written out where it is used.
///
/// [`err_line!`](macro@crate::err_line) under the name a warning is usually reported
/// with.
#[macro_export]
macro_rules! warn_line {
    ($prefix:expr => $($message:tt)+) => {
        $crate::warn_line(&$prefix, &$crate::__message!($($message)+))
    };
    ($($message:tt)+) => {
        $crate::warn_line("WARNING", &$crate::__message!($($message)+))
    };
}

/// The line a way out is offered on, written out where it is used.
///
/// [`err_line!`](macro@crate::err_line) under the name help is usually offered with.
#[macro_export]
macro_rules! help_line {
    ($prefix:expr => $($message:tt)+) => {
        $crate::help_line(&$prefix, &$crate::__message!($($message)+))
    };
    ($($message:tt)+) => {
        $crate::help_line("HELP", &$crate::__message!($($message)+))
    };
}

/// What a message written in the source says, given to the three lines above.
///
/// Not meant to be called: it is the one place that decides what a written message is,
/// so that the three do not each carry their own answer. A message written out is read
/// by [`format!`] — whether or not it turns out to hold anything to fill in — and one
/// the program already holds is taken as it stands.
#[doc(hidden)]
#[macro_export]
macro_rules! __message {
    // A message written out. This rule comes first because a lone literal matches the
    // one below as well, and it is the one that has to lose: a brace in a message is a
    // value to fill in, not a brace.
    ($format:literal $(, $argument:tt)*) => {
        ::std::format!($format $(, $argument)*)
    };
    // A message the program already holds.
    ($text:expr) => {
        ::std::string::ToString::to_string(&$text)
    };
}

/// Which line is being drawn, which is what decides its colour and its mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    /// Something that has gone wrong.
    Error,
    /// Something that is about to.
    Warning,
    /// Something the reader can do about it.
    Help,
}

impl Level {
    /// The colour the line is written in where the glyphs are missing.
    const fn colour(self) -> &'static str {
        match self {
            Self::Error => "RED",
            Self::Warning => "YELLOW",
            Self::Help => "green",
        }
    }

    /// The colour behind the badge, and the colour the badge is written in.
    const fn badge(self) -> (&'static str, &'static str) {
        match self {
            Self::Error => ("~RED", "#ffffff"),
            Self::Warning => ("~YELLOW", "black"),
            Self::Help => ("~green", "#ffffff"),
        }
    }

    /// The mark the badge is headed with.
    const fn mark(self) -> char {
        match self {
            Self::Error | Self::Warning => '\u{f071}',
            Self::Help => '\u{f02fc}',
        }
    }
}

/// Draws a line of `level`, for the theme the program is drawn with.
fn drawn(level: Level, prefix: &str, content: &str) -> String {
    TextRendering::render(&source(level, prefix, content, theme_choice()))
}

/// The color language a line of `level` is written in, for `theme`.
///
/// The two are not one line drawn in two ways: where the glyphs are missing the marking
/// is made out of letters, and where they are there it is made out of a mark on a
/// background. So the shape is chosen here, and how it is drawn is left to the engine.
fn source(level: Level, prefix: &str, content: &str, theme: ThemeChoice) -> String {
    let content = marked(content);

    if theme == ThemeChoice::Pretty {
        let (background, foreground) = level.badge();
        let mark = level.mark();
        format!("[[{background}]][[{foreground}]]** {mark} {prefix} **[[/]][[/]] {content}")
    } else {
        let colour = level.colour();
        format!("[[{colour}]]::{prefix}=>[[/]] {content}")
    }
}

/// The message, marked so that it reads as what the line says rather than as part of
/// the name it follows.
///
/// The marking is a **single-line** one. The language is read a line at a time, so a
/// pair of markers with a newline between them is not a pair at all: marking a message
/// of several lines would leave a stray `*` at each end of it. A message that is one
/// line is drawn in italics, and a longer one — a translation written as a `|` block,
/// or one that carries an example — is drawn as it is. What a `|` block ends with is
/// trimmed for the same reason it is not marking: it is not part of what was said.
fn marked(content: &str) -> String {
    let content = content.trim();

    if content.contains('\n') {
        content.to_string()
    } else {
        format!("*{content}*")
    }
}

#[cfg(test)]
mod tests {
    use crate::ThemeChoice;
    use crate::support::without_color;

    use super::{Level, err_line, source};

    /// What Rorolala's own messages are called.
    const PREFIX: &str = "ERROR";

    /// What they say.
    const CONTENT: &str = "Fail to load vault!";

    #[test]
    fn a_line_without_the_glyphs_is_letters() {
        assert_eq!(
            source(Level::Error, PREFIX, CONTENT, ThemeChoice::Simple),
            "[[RED]]::ERROR=>[[/]] *Fail to load vault!*"
        );
        assert_eq!(
            source(Level::Error, PREFIX, CONTENT, ThemeChoice::Text),
            "[[RED]]::ERROR=>[[/]] *Fail to load vault!*"
        );
        assert_eq!(
            source(Level::Warning, "WARNING", CONTENT, ThemeChoice::Simple),
            "[[YELLOW]]::WARNING=>[[/]] *Fail to load vault!*"
        );
        assert_eq!(
            source(Level::Help, "HELP", CONTENT, ThemeChoice::Simple),
            "[[green]]::HELP=>[[/]] *Fail to load vault!*"
        );
    }

    #[test]
    fn a_line_with_the_glyphs_is_a_badge() {
        assert_eq!(
            source(Level::Error, PREFIX, CONTENT, ThemeChoice::Pretty),
            "[[~RED]][[#ffffff]]** \u{f071} ERROR **[[/]][[/]] *Fail to load vault!*"
        );
        assert_eq!(
            source(Level::Warning, "WARNING", CONTENT, ThemeChoice::Pretty),
            "[[~YELLOW]][[black]]** \u{f071} WARNING **[[/]][[/]] *Fail to load vault!*"
        );
        assert_eq!(
            source(Level::Help, "HELP", CONTENT, ThemeChoice::Pretty),
            "[[~green]][[#ffffff]]** \u{f02fc} HELP **[[/]][[/]] *Fail to load vault!*"
        );
    }

    #[test]
    fn a_line_is_drawn_and_keeps_none_of_its_markings() {
        without_color(|| {
            for line in [
                err_line(PREFIX, CONTENT),
                super::warn_line("WARNING", CONTENT),
                super::help_line("HELP", CONTENT),
            ] {
                assert!(line.contains(CONTENT), "{line:?}");
                assert!(!line.contains("[[") && !line.contains('*'), "{line:?}");
            }
        });
    }

    #[test]
    fn a_written_line_says_what_a_call_says() {
        const LOAD: &str = "LOAD";

        without_color(|| {
            assert_eq!(
                crate::err_line!("Fail to load **{}**!", "vault"),
                err_line("ERROR", "Fail to load **vault**!")
            );
            assert_eq!(
                crate::err_line!(LOAD => "Fail to load {}!", "vault"),
                err_line("LOAD", "Fail to load vault!")
            );
            assert_eq!(
                crate::warn_line!("Fail to load vault!"),
                super::warn_line("WARNING", "Fail to load vault!")
            );
            assert_eq!(
                crate::help_line!("Fail to load vault!"),
                super::help_line("HELP", "Fail to load vault!")
            );
        });
    }

    #[test]
    fn a_written_line_takes_a_message_the_program_holds() {
        let held = String::from("Fail to load **vault**!");

        without_color(|| assert_eq!(crate::err_line!(held), err_line("ERROR", &held)));
    }

    #[test]
    fn a_message_a_block_scalar_ends_with_is_still_drawn() {
        // Every translation written as a `|` block in the locale files ends with a
        // newline, and the markers around it are on the far side of that newline.
        without_color(|| {
            assert_eq!(
                super::help_line("HELP", "Please try again\n"),
                "::HELP=> Please try again"
            );
        });
    }

    #[test]
    fn the_content_is_written_in_the_color_language_too() {
        without_color(|| {
            let line = err_line(PREFIX, "the `vault.json` is missing");
            assert_eq!(line, "::ERROR=> the `vault.json` is missing");
        });
    }
}
