#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use proc_macro::TokenStream;

// The copy below is kept byte for byte from the implementation it was taken from, so it
// stays diffable against it. That implementation does not deny the lints this workspace
// does, and its shape is what the lints object to: expanding placeholders over a token
// stream is one long, deeply nested walk (`too_many_lines`, `cognitive_complexity`), it
// builds identifiers by joining formatted parts (`uninlined_format_args`,
// `unnecessary_join`), and it has a `pub(crate)` entry point in a private module, which
// `rorolala-protocol` allows for the same reason.
#[allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    clippy::uninlined_format_args,
    clippy::unnecessary_join,
    clippy::option_if_let_else,
    clippy::redundant_pub_crate
)]
mod internal_repeat;

/// Emits a template body once per number in a range, rewriting the placeholders in
/// each copy.
///
/// Internal call signature: `internal_repeat!(1..=12 => { template })`. Inside the
/// template, `$` is the current number, `$-` is that number minus one, `$+` is that
/// number plus one, `^$` is the range's end and `$^` is its start; an identifier
/// followed by `$` gains the current number as a suffix (`T$-` becomes `T0`, `T1`, …).
/// A parenthesized group whose last token is `+` repeats once per number, with the
/// separator that precedes the `+` between the copies; a `^` after the closing paren
/// makes it repeat as many times as the range's end is away from the current number,
/// which is how a fixed-length list is padded.
#[proc_macro]
pub fn internal_repeat(input: TokenStream) -> TokenStream {
    internal_repeat::internal_repeat(input)
}
