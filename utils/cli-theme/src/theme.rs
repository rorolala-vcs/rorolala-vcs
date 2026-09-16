use std::io::{self, IsTerminal};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU8, Ordering};

use mingling::picker::Pickable;

/// How much a style may ask of the terminal it is drawn on
///
/// The three are ordered by how much that is, so a program picks the highest one it
/// knows both ends can render.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Pickable)]
pub enum ThemeChoice {
    /// ASCII content in one flat color
    ///
    /// The content of [`Simple`](Self::Simple), colored without the palette: what is
    /// left when a terminal carries color but nothing may vary with it.
    Text = 0,
    /// ASCII content in the standard colors
    ///
    /// Symbols are plain ASCII and colors come out of the sixteen a terminal is
    /// expected to have, so this is what every terminal renders as intended. It is also
    /// the default, which is what the picker names an absent argument by — not what a
    /// program is drawn in, which is the global and starts as the terminal's answer.
    #[default]
    Simple = 1,
    /// `NerdFont` glyphs in true color
    ///
    /// Asks for a font whose glyphs are not ASCII, and for 24-bit color. Neither can be
    /// asked of a terminal, so this is only ever chosen by hand.
    Pretty = 2,
}

impl ThemeChoice {
    /// The most a terminal is known to carry here
    ///
    /// [`Pretty`](Self::Pretty) is never returned: nothing can tell whether a font has
    /// the glyphs or a terminal the depth, so asking for it stays a person's decision.
    /// What is left is between the two that only use ASCII — and a terminal that is not
    /// being written to gets the one that assumes least of it.
    #[must_use]
    pub fn auto_select() -> Self {
        if io::stdout().is_terminal() {
            Self::Simple
        } else {
            Self::Text
        }
    }

    /// This choice as the number the global holds
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as Self as u8
    }

    /// The choice a number stands for
    ///
    /// A number that names no choice — which only a caller writing the global directly
    /// can produce — reads as [`Simple`](Self::Simple), the one that assumes a terminal
    /// is at least ordinary.
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Text,
            2 => Self::Pretty,
            _ => Self::Simple,
        }
    }
}

/// Which theme Rorolala's output is drawn in
///
/// Starts as the terminal's answer — see [`theme_choice`] — and is replaced the moment
/// a program states its own, which is why it is lazily built rather than holding a
/// number from the start: the initial value is not knowable until the terminal is asked.
static CHOICE: LazyLock<AtomicU8> =
    LazyLock::new(|| AtomicU8::new(ThemeChoice::auto_select() as u8));

/// States which theme the program is drawn in, for the whole program
pub fn set_theme_choice(choice: ThemeChoice) {
    CHOICE.store(choice.as_u8(), Ordering::Relaxed);
}

/// Which theme the program is drawn in
///
/// Until [`set_theme_choice`] is called, this is [`ThemeChoice::auto_select`]'s answer.
#[must_use]
pub fn theme_choice() -> ThemeChoice {
    ThemeChoice::from_u8(CHOICE.load(Ordering::Relaxed))
}
