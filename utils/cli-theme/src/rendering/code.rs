//! Fenced code, drawn by `bat`.

use bat::{PrettyPrinter, WrappingMode};

use crate::theme::ThemeChoice;

use super::ansi::{strip, to_sixteen};
use super::palette::RenderOptions;

/// How wide `bat` is told the terminal is.
///
/// `bat` needs a width to lay a snippet out against, and asks the terminal for one
/// when it has not been given it — which a library has no business doing, because the
/// answer decides what comes out. Nothing is wrapped here, so the number only has to
/// be large enough not to matter.
const WIDTH: usize = 200;

/// How wide a tab is drawn as.
const TAB_WIDTH: usize = 4;

/// Draws `code` as the language it names.
///
/// `bat` is given the code rather than a file, so it has nothing to guess from: the
/// language is whatever the fence said, and a name `bat` does not know is drawn as
/// plain code rather than refused. If even that fails, the code is handed back as it
/// came in — a message is worth more than its highlighting.
#[must_use]
pub(super) fn highlight(code: &str, language: Option<&str>, options: RenderOptions) -> String {
    let Some(drawn) = language
        .and_then(|language| run(code, Some(language)))
        .or_else(|| run(code, None))
    else {
        return code.to_owned();
    };
    let drawn = drawn.trim_end_matches('\n');

    if !options.color || options.theme == ThemeChoice::Text {
        strip(drawn)
    } else if options.theme == ThemeChoice::Pretty {
        drawn.to_owned()
    } else {
        to_sixteen(drawn)
    }
}

/// Runs `bat` once, and reports what it drew.
fn run(code: &str, language: Option<&str>) -> Option<String> {
    let mut printer = PrettyPrinter::new();
    printer
        .input_from_bytes(code.as_bytes())
        .colored_output(true)
        .true_color(true)
        .header(false)
        .line_numbers(false)
        .grid(false)
        .rule(false)
        .snip(false)
        .show_nonprintable(false)
        .use_italics(true)
        .tab_width(Some(TAB_WIDTH))
        .wrapping_mode(WrappingMode::NoWrapping(true))
        .term_width(WIDTH)
        .theme(theme());
    if let Some(language) = language {
        printer.language(language);
    }

    let mut drawn = String::new();
    printer.print_with_writer(Some(&mut drawn)).ok()?;
    Some(drawn)
}

/// The theme `bat` draws with.
///
/// Named rather than left to `bat`, which would read the environment and the
/// terminal's background for one: a library that answers differently depending on the
/// machine it runs on is a library whose output cannot be checked.
fn theme() -> String {
    let options = bat::theme::ThemeOptions {
        theme: bat::theme::ThemePreference::Dark,
        ..bat::theme::ThemeOptions::default()
    };
    bat::theme::theme(options).to_string()
}
