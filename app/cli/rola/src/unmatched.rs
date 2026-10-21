//! The words a command was given and has no use for.
//!
//! A command picks the arguments it knows, and what is left is what the caller did not mean to
//! say: a misspelt flag, a word for another command. A run that has any of it has been
//! misunderstood, so the words are named and the run is refused.
//!
//! What is drawn is the command as it was written, with each word that picked nothing marked
//! under it: the frame — the `-->` that heads it and the `|` beside every line — in bright blue,
//! the words the command did use and the heading in white, and the words it did not, the carets
//! under them and what those carets say in bright magenta. Those are the colours a drawing of
//! this program is made of.
//!
//! The refusal is a failure like any other, so that what is drawn is drawn by its renderer: the
//! `--json` answer a program reads keeps being one JSON document with nothing beside it, and a
//! run drawn with a theme that has no colour is drawn without one. Nothing here writes to the
//! terminal on its own.
//!
//! `arg-picker` draws the same kind of snippet, in English and in a palette of its own; this is
//! the program's own drawing, so that the words are the ones the rest of the program is
//! translated into and the colours are Rorolala's.

use annotate_snippets::{AnnotationKind, Level, Renderer, Snippet};
use mingling::picker::UnmatchedArgs;
use mingling::{
    Grouped,
    macros::{buffer, r_eprintln, renderer},
    res::ResExitCode,
};
use rorolala_errors::Failure;
use rorolala_utils_cli_theme::{Colorize as _, help_line};
use rust_i18n::t;

use crate::Next;
use crate::exit_codes::EC_ERR_UNMATCHED;
use crate::failure::failure;

/// The line-number column a renderer always writes, and the plain gutter that reads better.
const LINED: &str = "1 | ";

/// The gutter every line of the snippet is drawn beside.
const GUTTER: &str = "  |";

/// What heads the snippet.
const HEAD: &str = "-->";

/// The snippet naming every word of `unmatched`, or nothing when every word was used.
///
/// It is drawn plain and then coloured, because the renderer writes a palette of its own and this
/// program's is the one above. The words are drawn from the argument list rather than from the
/// renderer's copy of it, so that a word the command used and a word it did not can be told apart
/// on the same line.
#[must_use]
pub fn drawn(unmatched: &UnmatchedArgs) -> String {
    if unmatched.is_empty() {
        return String::new();
    }

    let original = unmatched.original();
    let source = original.join(" ");
    let title = t!("unmatched.title").trim().to_owned();
    let label = t!("unmatched.label").trim().to_owned();

    // Where each word starts in the one line the words are drawn as, so that a mark lands under
    // the word it names even when the words are of different lengths.
    let mut starts = Vec::with_capacity(original.len());
    let mut offset = 0;
    for word in original {
        starts.push(offset);
        offset += word.len() + 1;
    }

    let mut snippet = Snippet::source(&source).line_start(1);
    for entry in unmatched {
        if let Some(&start) = starts.get(entry.index) {
            snippet =
                snippet.annotation(AnnotationKind::Primary.span(start..start + entry.raw.len()));
        }
    }

    let report = [Level::ERROR
        .no_name()
        .primary_title(title.as_str())
        .element(snippet)];
    let rendered = strip_line_number(&Renderer::plain().render(&report));

    painted(&rendered, original, unmatched, &label)
}

/// `rendered` with the line-number column turned into the plain gutter.
fn strip_line_number(rendered: &str) -> String {
    let mut out = String::with_capacity(rendered.len());
    for line in rendered.lines() {
        match line.strip_prefix(LINED) {
            Some(rest) => {
                out.push_str(GUTTER);
                out.push_str(rest);
            }
            None => out.push_str(line),
        }
        out.push('\n');
    }
    out
}

/// `rendered` drawn in the colours a drawing of this program is made of.
///
/// The line the words are on is written again from `original`, since the renderer's copy says
/// nothing about which words were used: a word the command picked is white and one it did not is
/// bright magenta, and the carets under the latter say the same in the same colour.
///
/// The words the marks say are named once, at the end of the line the carets are on, rather than
/// by the renderer: a renderer naming a word per caret puts the second one under the first, and
/// what is wanted is one line saying what every mark is for.
fn painted(rendered: &str, original: &[String], unmatched: &UnmatchedArgs, label: &str) -> String {
    let mut marked = vec![false; original.len()];
    for entry in unmatched {
        if let Some(said) = marked.get_mut(entry.index) {
            *said = true;
        }
    }

    let mut out = String::with_capacity(rendered.len() + rendered.len() / 2);
    for line in rendered.lines() {
        // The first line is the heading, which the renderer writes as words alone: what heads
        // this program's drawings is `-->`, and the words are the drawing's to name.
        if out.is_empty() {
            let said = line
                .trim_start()
                .strip_prefix(HEAD)
                .map_or_else(|| line.trim_start(), str::trim);
            out.push_str(&HEAD.bright_blue().to_string());
            out.push(' ');
            out.push_str(&said.white().to_string());
            out.push('\n');
            continue;
        }

        // The gutter on its own says nothing: it opens the snippet, and the drawing ends with the
        // carets and what they say rather than closing again.
        if line == GUTTER {
            if out.is_empty() {
                out.push_str(&line.bright_blue().to_string());
                out.push('\n');
            }
            continue;
        }

        // The words as they were written, each in the colour that says whether the command used
        // it. The renderer's own line is the same characters, so this is the same line drawn.
        if line.starts_with(GUTTER) && !line.contains('^') {
            out.push_str(&GUTTER.bright_blue().to_string());
            out.push(' ');
            for (at, word) in original.iter().enumerate() {
                if at > 0 {
                    out.push(' ');
                }
                let drawn = if marked[at] {
                    word.bright_magenta().to_string()
                } else {
                    word.white().to_string()
                };
                out.push_str(&drawn);
            }
            out.push('\n');
            continue;
        }

        // The carets, which mark the words, and what they say: the mark itself rather than the
        // frame, in one colour.
        match line.strip_prefix(GUTTER) {
            Some(rest) if rest.contains('^') => {
                out.push_str(&GUTTER.bright_blue().to_string());
                out.push_str(&rest.bright_magenta().to_string());
                out.push(' ');
                out.push_str(&label.bright_magenta().to_string());
            }
            Some(rest) => {
                out.push_str(&GUTTER.bright_blue().to_string());
                out.push_str(&rest.bright_magenta().to_string());
            }
            None => out.push_str(&line.bright_magenta().to_string()),
        }
        out.push('\n');
    }
    out
}

/// Refuses the run whose command was left with words it does not know.
///
/// `command` is the command as `--help` is asked of it — `layout new` — so that the way out the
/// drawing names is the one the words were given to. A command with no word left is answered with
/// nothing, and the run goes on.
#[must_use]
pub fn refused(words: &UnmatchedArgs, command: &str) -> Option<Next> {
    if words.is_empty() {
        return None;
    }

    Some(
        ErrorUnmatched {
            words: words.clone(),
            command: command.to_owned(),
        }
        .into(),
    )
}

/// Error: a command was given words it has no use for.
///
/// The words are what the drawing marks, so they travel with the failure rather than being said
/// where the command was written: a failure is what a run is drawn from, and a run that says what
/// is wrong on its own is one whose `--json` answer is no longer one document.
#[derive(Grouped)]
pub struct ErrorUnmatched {
    /// The words none of the command's picks consumed.
    pub words: UnmatchedArgs,
    /// The command they were given to, as `--help` is asked of it.
    pub command: String,
}

impl Failure for ErrorUnmatched {
    fn name(&self) -> &'static str {
        "error_unmatched"
    }

    fn reason(&self) -> String {
        t!("unmatched.title").trim().to_string()
    }
}

failure!(ErrorUnmatched);

#[renderer(buffer)]
pub fn render_error_unmatched(error: ErrorUnmatched, ec: &mut ResExitCode) {
    r_eprintln!("{}", drawn(&error.words).trim_end());
    r_eprintln!(
        "{}",
        help_line!(t!("unmatched.help", command = error.command).trim())
    );
    ec.exit_code = EC_ERR_UNMATCHED;
}

#[cfg(test)]
mod tests {
    use mingling::macros::arg;
    use mingling::picker::{IntoPicker as _, UnmatchedArgs, consts::UNMATCHED};

    use super::drawn;

    /// The words no pick consumed out of `args`, picked as a command picks them.
    fn unmatched_of(args: &[&str]) -> UnmatchedArgs {
        let picked = args
            .to_vec()
            .pick(arg![name: Option<String>])
            .pick(&UNMATCHED)
            .unwrap();
        picked.1
    }

    #[test]
    fn every_word_that_picked_nothing_is_named_once() {
        let unmatched = unmatched_of(&["--name", "alice", "extra", "--nonsense"]);

        assert_eq!(unmatched.len(), 2);

        let drawn = drawn(&unmatched);
        assert!(drawn.contains("extra"), "{drawn}");
        assert!(drawn.contains("--nonsense"), "{drawn}");
        // One line says what every mark is for, rather than one per mark.
        assert_eq!(drawn.matches("unknown argument").count(), 1, "{drawn}");
    }

    #[test]
    fn nothing_is_drawn_when_every_word_was_used() {
        assert_eq!(drawn(&unmatched_of(&["--name", "alice"])), String::new());
    }

    #[test]
    fn the_frame_is_blue_a_used_word_is_white_and_a_mark_is_magenta() {
        rorolala_utils_cli_theme::set_enabled(true);
        let drawn = drawn(&unmatched_of(&["--name", "alice", "unused"]));
        rorolala_utils_cli_theme::unset_enabled();

        let blue = "\u{1b}[94m";
        let white = "\u{1b}[37m";
        let magenta = "\u{1b}[95m";
        let plain = "\u{1b}[0m";

        // The heading is the frame's symbol and the drawing's own words.
        assert!(drawn.contains(&format!("{blue}-->")), "{drawn:?}");
        assert!(
            drawn.contains(&format!("{white}the command was given")),
            "{drawn:?}"
        );

        // Every word of the line is drawn, in the colour that says whether it was used.
        assert!(drawn.contains(&format!("{white}--name")), "{drawn:?}");
        assert!(drawn.contains(&format!("{white}alice")), "{drawn:?}");
        assert!(drawn.contains(&format!("{magenta}unused")), "{drawn:?}");

        // The carets — and what they say — are the mark, beside a gutter that stays frame.
        assert!(
            drawn.contains(&format!("{blue}  |{plain}{magenta}")),
            "{drawn:?}"
        );
        assert!(
            drawn.contains(&format!("{magenta}unknown argument")),
            "{drawn:?}"
        );
    }
}
