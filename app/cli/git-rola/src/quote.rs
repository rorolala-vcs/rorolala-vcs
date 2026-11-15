//! How a path is written out, the way git writes one.
//!
//! git quotes a path that a reader could not take at face value: a byte that does not print, a
//! quote, a backslash, and, unless `core.quotePath` is false, every byte of a character outside
//! ASCII. The escaping is per byte, so the three bytes of `素` become three escapes rather than one
//! character's worth, which is what git has always written and what a reader of its output expects.
//!
//! The setting is read once and kept: it cannot change while a run lasts, and `check` asks for it
//! once per path it prints.

use std::sync::OnceLock;

/// Whether `core.quotePath` is on, as git reads it when a run starts.
static QUOTE_PATH: OnceLock<bool> = OnceLock::new();

/// The bytes a path is written with, as git writes them.
///
/// A path that needs no quoting is returned as it is, since the quotes are what tells a reader that
/// something had to be escaped.
pub fn path(text: &str) -> String {
    quoted(text, quotable())
}

/// Whether paths are quoted past ASCII, read from `core.quotePath` the first time it is asked.
fn quotable() -> bool {
    *QUOTE_PATH.get_or_init(|| crate::git_config::boolean("core.quotePath").unwrap_or(true))
}

/// `text` quoted as git quotes it, with `quote_path` naming whether bytes past ASCII are escaped.
fn quoted(text: &str, quote_path: bool) -> String {
    let needs_quoting = text
        .as_bytes()
        .iter()
        .any(|byte| needs_escape(*byte, quote_path));

    if !needs_quoting {
        return text.to_owned();
    }

    let mut out = Vec::with_capacity(text.len() + 2);
    out.push(b'"');

    for byte in text.bytes() {
        match byte {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            0x07 => out.extend_from_slice(b"\\a"),
            0x08 => out.extend_from_slice(b"\\b"),
            b'\t' => out.extend_from_slice(b"\\t"),
            b'\n' => out.extend_from_slice(b"\\n"),
            0x0b => out.extend_from_slice(b"\\v"),
            0x0c => out.extend_from_slice(b"\\f"),
            b'\r' => out.extend_from_slice(b"\\r"),
            // Everything else that needs quoting is a control byte or a byte of a character
            // outside ASCII, and both are written as three octal digits of the byte. `quote_path`
            // is what decides whether the latter needs it, and `needs_escape` has already said
            // this byte is one of the two.
            other if needs_escape(other, quote_path) => {
                out.extend_from_slice(format!("\\{other:03o}").as_bytes());
            }
            other => out.push(other),
        }
    }

    out.push(b'"');

    // The bytes came from a `str` and every escape written is ASCII, so what is written is still
    // UTF-8; the lossy reading is only there not to panic on a path that would not be.
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether `byte` has to be written as an escape.
fn needs_escape(byte: u8, quote_path: bool) -> bool {
    matches!(byte, b'"' | b'\\' | 0x07..=0x0d)
        || (byte < 0x20 || byte == 0x7f)
        || (quote_path && byte >= 0x80)
}

#[cfg(test)]
mod tests {
    use super::quoted;

    #[test]
    fn a_path_that_needs_nothing_is_written_as_it_is() {
        assert_eq!(quoted("Assets/hero.psd", true), "Assets/hero.psd");
        assert_eq!(quoted("a b.psd", true), "a b.psd");
    }

    #[test]
    fn a_character_outside_ascii_is_escaped_by_its_bytes() {
        assert_eq!(
            quoted("素材.psd", true),
            "\"\\347\\264\\240\\346\\235\\220.psd\""
        );
    }

    #[test]
    fn quote_path_off_leaves_the_bytes_alone() {
        assert_eq!(quoted("素材.psd", false), "素材.psd");
    }

    #[test]
    fn a_control_byte_is_escaped_even_with_quote_path_off() {
        assert_eq!(quoted("a\tb", false), "\"a\\tb\"");
        assert_eq!(quoted("a\nb", false), "\"a\\nb\"");
        assert_eq!(quoted("a\x01b", false), "\"a\\001b\"");
    }

    #[test]
    fn a_quote_and_a_backslash_are_escaped() {
        assert_eq!(quoted("a\"b", false), "\"a\\\"b\"");
        assert_eq!(quoted("a\\b", false), "\"a\\\\b\"");
    }
}
