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
/// The line [`err_line`] draws, in the colour a warning is drawn in.
#[must_use]
pub fn warn_line(prefix: &str, content: &str) -> String {
    drawn(Level::Warning, prefix, content)
}

/// The line a way out is offered on.
///
/// The line [`err_line`] draws, in the colour help is drawn in.
#[must_use]
pub fn help_line(prefix: &str, content: &str) -> String {
    drawn(Level::Help, prefix, content)
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
    if theme == ThemeChoice::Pretty {
        let (background, foreground) = level.badge();
        let mark = level.mark();
        format!("[[{background}]][[{foreground}]]** {mark} {prefix} **[[/]][[/]] *{content}*")
    } else {
        let colour = level.colour();
        format!("[[{colour}]]::{prefix}=>[[/]] *{content}*")
    }
}

#[cfg(test)]
mod tests {
    use crate::ThemeChoice;

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
        for line in [
            err_line(PREFIX, CONTENT),
            super::warn_line("WARNING", CONTENT),
            super::help_line("HELP", CONTENT),
        ] {
            assert!(line.contains(CONTENT), "{line:?}");
            assert!(!line.contains("[[") && !line.contains('*'), "{line:?}");
        }
    }

    #[test]
    fn the_content_is_written_in_the_color_language_too() {
        let line = err_line(PREFIX, "the `vault.json` is missing");
        assert_eq!(line, "::ERROR=> the `vault.json` is missing");
    }
}
