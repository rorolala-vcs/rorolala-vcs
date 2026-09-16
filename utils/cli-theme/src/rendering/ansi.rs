//! Escape sequences: what a terminal reads instead of printing, and how wide what it
//! does print comes out.

use unicode_width::UnicodeWidthStr as _;

use super::style::{NamedColor, Rgb, nearest};

/// The byte that introduces an escape sequence.
const ESCAPE: u8 = 0x1b;

/// A piece of a string, split at its escape sequences.
#[derive(Debug, Clone, Copy)]
enum Segment<'a> {
    /// Text a terminal prints.
    Text(&'a str),
    /// A `ESC [ ... m` sequence, which says how the text after it is drawn.
    Sgr(&'a str),
    /// Any other escape sequence, which this module does not read.
    Other(&'a str),
}

/// Splits `text` at every escape sequence it holds.
fn segments(text: &str) -> Vec<Segment<'_>> {
    let mut parts = Vec::new();
    let mut plain = 0;
    let mut index = 0;
    while index < text.len() {
        if text.as_bytes()[index] != ESCAPE {
            index += 1;
            continue;
        }
        if plain < index {
            parts.push(Segment::Text(&text[plain..index]));
        }
        let end = sequence_end(text, index);
        let sequence = &text[index..end];
        parts.push(if is_sgr(sequence) {
            Segment::Sgr(sequence)
        } else {
            Segment::Other(sequence)
        });
        index = end;
        plain = end;
    }
    if plain < text.len() {
        parts.push(Segment::Text(&text[plain..]));
    }
    parts
}

/// Whether `sequence` is a `ESC [ ... m` sequence.
fn is_sgr(sequence: &str) -> bool {
    sequence.starts_with("\u{1b}[") && sequence.ends_with('m')
}

/// The index just past the escape sequence starting at `start`.
fn sequence_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    match bytes.get(start + 1) {
        // A control sequence ends with a byte in `@`..=`~`.
        Some(b'[') => {
            let mut index = start + 2;
            while index < bytes.len() && !(0x40..=0x7e).contains(&bytes[index]) {
                index += 1;
            }
            (index + 1).min(bytes.len())
        }
        // An operating system command ends with `BEL` or with `ESC \`.
        Some(b']') => {
            let mut index = start + 2;
            while index < bytes.len() {
                if bytes[index] == 0x07 {
                    return index + 1;
                }
                if bytes[index] == ESCAPE && bytes.get(index + 1) == Some(&b'\\') {
                    return index + 2;
                }
                index += 1;
            }
            bytes.len()
        }
        // Everything else is a two byte sequence.
        Some(_) => (start + 2).min(bytes.len()),
        None => bytes.len(),
    }
}

/// The width `text` takes up on screen.
///
/// Escape sequences take up none, whichever kind they are, so a highlighted line is
/// measured by the line rather than by the escapes drawn around it.
#[must_use]
pub(super) fn display_width(text: &str) -> usize {
    segments(text)
        .iter()
        .map(|segment| match segment {
            Segment::Text(plain) => plain.width(),
            Segment::Sgr(_) | Segment::Other(_) => 0,
        })
        .sum()
}

/// How many escape sequences `text` holds.
///
/// A table is measured in columns by `prettytable`, which counts one column for every
/// sequence it finds and leaves the `ESC` itself in the width; the count is what
/// [`super::table`] evens out so that the measurement cancels. Only `ESC [ ... m`
/// counts: a link's `ESC ] ...` is left out of tables entirely.
#[must_use]
pub(super) fn escape_count(text: &str) -> usize {
    segments(text)
        .iter()
        .filter(|segment| matches!(segment, Segment::Sgr(_)))
        .count()
}

/// `text` with every escape sequence taken out.
///
/// A syntax highlighter draws from its own palette, which is twenty-four bit, so the
/// flat theme has nothing to keep from it — and it keeps nothing at all, not even the
/// colour: [`paint`] draws nothing there either, so a bold word would be the only
/// emphasis left on the screen.
///
/// [`paint`]: super::style::paint
#[must_use]
pub(super) fn strip(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for segment in segments(text) {
        if let Segment::Text(plain) = segment {
            out.push_str(plain);
        }
    }
    out
}

/// `text` with every colour reduced to the nearest of the sixteen.
///
/// The theme with the sixteen is the one a terminal carries when it carries colour but
/// not much of it, and a highlighter's palette is not one the sixteen can spell. The
/// colours are moved rather than dropped, because a highlight nobody can see is worse
/// than a highlight that is nearly right.
#[must_use]
pub(super) fn to_sixteen(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for segment in segments(text) {
        match segment {
            Segment::Text(plain) | Segment::Other(plain) => out.push_str(plain),
            Segment::Sgr(sequence) => out.push_str(&rewrite(sequence)),
        }
    }
    out
}

/// `sequence` with the colours it names reduced to the nearest of the sixteen.
fn rewrite(sequence: &str) -> String {
    let Some(parameters) = sequence
        .strip_prefix("\u{1b}[")
        .and_then(|rest| rest.strip_suffix('m'))
    else {
        return sequence.to_owned();
    };

    let tokens: Vec<&str> = parameters.split(';').collect();
    let mut kept: Vec<String> = Vec::new();
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index];
        // An extended colour is the only one that has to move. Everything else is
        // carried over as it stands, including the colours that are already one of the
        // sixteen — a sequence that says `31` is a sequence the sixteen can already
        // draw, and leaving it out would leave whatever colour was in force before it.
        if (token == "38" || token == "48")
            && let Some((color, consumed)) = extended(&tokens, index)
        {
            kept.push(parameter(token == "48", color));
            index += consumed;
            continue;
        }
        kept.push(token.to_owned());
        index += 1;
    }

    if kept.is_empty() {
        return String::new();
    }
    format!("\u{1b}[{}m", kept.join(";"))
}

/// The parameter that asks for `color`, drawn behind the text when `background`.
fn parameter(background: bool, color: Rgb) -> String {
    let index = nearest(color).index();
    if background {
        if index < 8 {
            format!("4{index}")
        } else {
            format!("10{}", index - 8)
        }
    } else if index < 8 {
        format!("3{index}")
    } else {
        format!("9{}", index - 8)
    }
}

/// The colour `tokens[index]` reaches through the `38`/`48` form, and how many tokens
/// that took up.
fn extended(tokens: &[&str], index: usize) -> Option<(Rgb, usize)> {
    match *tokens.get(index + 1)? {
        "5" => {
            let value: u8 = tokens.get(index + 2)?.parse().ok()?;
            Some((indexed(value), 3))
        }
        "2" => {
            let red = tokens.get(index + 2)?.parse().ok()?;
            let green = tokens.get(index + 3)?.parse().ok()?;
            let blue = tokens.get(index + 4)?.parse().ok()?;
            Some((Rgb { red, green, blue }, 5))
        }
        _ => None,
    }
}

/// The colour a number in the two hundred and fifty-six colour palette stands for.
fn indexed(value: u8) -> Rgb {
    match value {
        0..=15 => NamedColor::from_index(value).rgb(),
        16..=231 => {
            let step = value - 16;
            let level = |amount: u8| if amount == 0 { 0 } else { 55 + amount * 40 };
            Rgb {
                red: level(step / 36),
                green: level((step / 6) % 6),
                blue: level(step % 6),
            }
        }
        _ => {
            let level = 8 + (value - 232) * 10;
            Rgb {
                red: level,
                green: level,
                blue: level,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{display_width, strip, to_sixteen};

    #[test]
    fn an_escape_sequence_takes_up_no_width() {
        assert_eq!(display_width("ab"), 2);
        assert_eq!(display_width("\u{1b}[31mab\u{1b}[0m"), 2);
        // `ESC ] 8 ; ; url` ... `ESC ] 8 ; ;`, which is how a link is handed to a
        // terminal. Either `BEL` or `ESC \` ends one, and `BEL` is written here
        // because it is the one that needs no backslash.
        let link = "\u{1b}]8;;https://example.com\u{7}ab\u{1b}]8;;\u{7}";
        assert_eq!(display_width(link), 2);
        // A wide character still measures as two.
        assert_eq!(display_width("\u{4e00}\u{4e00}"), 4);
    }

    #[test]
    fn stripping_leaves_what_a_terminal_prints() {
        assert_eq!(strip("\u{1b}[1;31mred\u{1b}[0m"), "red");
    }

    #[test]
    fn a_colour_moves_to_the_nearest_of_the_sixteen() {
        assert_eq!(
            to_sixteen("\u{1b}[38;2;255;0;0mred\u{1b}[0m"),
            "\u{1b}[91mred\u{1b}[0m"
        );
        assert_eq!(
            to_sixteen("\u{1b}[48;2;255;0;0mred\u{1b}[0m"),
            "\u{1b}[101mred\u{1b}[0m"
        );
        // The two hundred and fifty-six colour palette reaches the same place.
        assert_eq!(to_sixteen("\u{1b}[38;5;9mred"), "\u{1b}[91mred");
    }

    #[test]
    fn a_colour_the_sixteen_already_hold_is_left_alone() {
        assert_eq!(
            to_sixteen("\u{1b}[90mred\u{1b}[0m"),
            "\u{1b}[90mred\u{1b}[0m"
        );
        assert_eq!(to_sixteen("\u{1b}[1mred\u{1b}[0m"), "\u{1b}[1mred\u{1b}[0m");
    }
}
