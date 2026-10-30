use std::env;
use std::io::{self, IsTerminal};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU8, Ordering};

use mingling::picker::Pickable;

/// What the output is drawn for
///
/// Three of these are about the terminal the output is drawn on, ordered by how much a
/// style may ask of it, so a program picks the highest one it knows both ends can
/// render. The fourth is not a terminal at all — see
/// [`DesktopFriendly`](Self::DesktopFriendly) — and is never arrived at by choosing.
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
    /// Asks for a font whose glyphs are not ASCII, and for 24-bit color. A font cannot be
    /// asked of a terminal, so [`auto_select`](Self::auto_select) approximates the answer:
    /// it picks this only where the environment says the color depth is there and the
    /// terminal is one of the modern ones, and every run can still be told otherwise with
    /// `--theme-choice simple`.
    Pretty = 2,
    /// Marked lines for a program to read rather than a terminal to draw
    ///
    /// What reports something says which it is in a word at the head of the line — `err: `,
    /// `warn: `, `help: ` — and what it says follows as it was written, its marks and all, because
    /// what draws it on the other end reads the same language this program writes in. What is not a
    /// line of one of those is written exactly as it was: a result is a result, and the other end
    /// shows it as text.
    ///
    /// It is not a fourth step in what a terminal carries but the one choice about a reader that is
    /// not a terminal, which is why [`auto_select`](Self::auto_select) never answers it.
    DesktopFriendly = 3,
}

impl ThemeChoice {
    /// The most a terminal is known to carry here
    ///
    /// A terminal that is not being written to gets [`Text`](Self::Text). A terminal that
    /// is, and whose environment names one of the modern terminals — Windows Terminal,
    /// Zed, `WezTerm`, kitty, Ghostty, iTerm and the rest of the list — gets
    /// [`Pretty`](Self::Pretty); every other terminal gets [`Simple`](Self::Simple).
    ///
    /// The `Pretty` test is an approximation on purpose: a terminal carries no answer
    /// about its font, so this reads what the environment does say. A listed terminal is
    /// trusted for the color depth as well as the glyphs, since the list is made of
    /// 24-bit terminals; `COLORTERM` and `TERM` are read for a terminal that names itself
    /// some other way, which can only lower the choice, never raise it. A terminal that
    /// is not on the list, or a run that asks for another theme, is answered as it always
    /// was.
    #[must_use]
    pub fn auto_select() -> Self {
        if !io::stdout().is_terminal() {
            return Self::Text;
        }

        if draws_pretty() {
            Self::Pretty
        } else {
            Self::Simple
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
            3 => Self::DesktopFriendly,
            _ => Self::Simple,
        }
    }
}

/// Whether the terminal this program writes to is known to carry what [`Pretty`](ThemeChoice::Pretty) asks for.
///
/// It reads the environment rather than the terminal because a terminal carries no answer about its
/// font: `COLORTERM` and `TERM` are asked for the color depth, and the variables a modern terminal
/// is known to set are asked for which one it is. A terminal that names neither is left at
/// [`Simple`](ThemeChoice::Simple), where the glyphs are ASCII and the colors are the sixteen every
/// terminal has.
fn draws_pretty() -> bool {
    let colorterm = env::var("COLORTERM").ok();
    let term = env::var("TERM").ok();
    let program = env::var("TERM_PROGRAM").ok();
    let markers = [
        env::var_os("WT_SESSION").is_some(),
        env::var_os("KITTY_WINDOW_ID").is_some(),
        env::var_os("WEZTERM_EXECUTABLE").is_some(),
        env::var_os("GHOSTTY_RESOURCES_DIR").is_some(),
        env::var_os("ZED_TERM").is_some(),
    ];

    draws_pretty_for(
        colorterm.as_deref(),
        term.as_deref(),
        program.as_deref(),
        &markers,
    )
}

/// The answer [`draws_pretty`] reads the process's environment for.
fn draws_pretty_for(
    colorterm: Option<&str>,
    term: Option<&str>,
    program: Option<&str>,
    markers: &[bool],
) -> bool {
    let modern = modern_terminal(program, term, markers);

    modern && has_true_color(colorterm, term, modern)
}

/// Whether the terminal draws 24-bit color, or is one of the terminals known to.
///
/// A listed terminal is trusted for the depth as well as the glyphs: Windows Terminal, Zed and the
/// others are 24-bit terminals, and requiring `COLORTERM` from them would be asking them to repeat
/// what their own name already says. `COLORTERM` and `TERM` are still read for the terminals that
/// name themselves some other way.
fn has_true_color(colorterm: Option<&str>, term: Option<&str>, modern: bool) -> bool {
    modern || true_color(colorterm, term)
}

/// Whether the environment says the terminal draws 24-bit color.
fn true_color(colorterm: Option<&str>, term: Option<&str>) -> bool {
    let colorterm = colorterm.unwrap_or_default().to_ascii_lowercase();
    let term = term.unwrap_or_default().to_ascii_lowercase();

    colorterm.contains("truecolor") || colorterm.contains("24bit") || term.contains("direct")
}

/// Whether the environment names one of the terminals `Pretty` is drawn for.
///
/// The list is the terminals whose users have a `NerdFont` often enough to be worth guessing, not
/// every terminal that could ever show one: a terminal missing from it is still answerable with
/// `--theme-choice pretty`.
fn modern_terminal(program: Option<&str>, term: Option<&str>, markers: &[bool]) -> bool {
    if markers.iter().any(|set| *set) {
        return true;
    }

    let program = program.unwrap_or_default().to_ascii_lowercase();
    if matches!(
        program.as_str(),
        "wezterm" | "iterm.app" | "kitty" | "ghostty" | "vscode" | "hyper" | "zed"
    ) {
        return true;
    }

    let term = term.unwrap_or_default().to_ascii_lowercase();

    ["xterm-kitty", "wezterm", "ghostty", "alacritty", "foot"]
        .iter()
        .any(|name| {
            term == *name
                || term
                    .strip_prefix(name)
                    .is_some_and(|rest| rest.starts_with('-'))
        })
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

#[cfg(test)]
mod tests {
    use super::{draws_pretty_for, modern_terminal, true_color};

    #[test]
    fn a_color_depth_is_read_from_the_environment() {
        assert!(true_color(Some("truecolor"), None));
        assert!(true_color(Some("24bit"), None));
        assert!(true_color(None, Some("xterm-direct")));
        assert!(!true_color(Some("256"), None));
        assert!(!true_color(None, None));
    }

    #[test]
    fn a_modern_terminal_is_read_from_what_it_says_about_itself() {
        assert!(modern_terminal(Some("WezTerm"), None, &[]));
        assert!(modern_terminal(Some("iTerm.app"), None, &[]));
        assert!(modern_terminal(Some("vscode"), None, &[]));
        assert!(modern_terminal(Some("zed"), None, &[]));
        assert!(modern_terminal(None, Some("xterm-kitty"), &[]));
        assert!(modern_terminal(
            None,
            Some("xterm-256color"),
            &[false, false, false, false, true]
        ));
        assert!(!modern_terminal(
            Some("Apple_Terminal"),
            Some("xterm-256color"),
            &[]
        ));
        assert!(!modern_terminal(None, Some("xterm-256color"), &[]));
    }

    #[test]
    fn pretty_needs_a_terminal_it_is_drawn_for_and_a_depth_it_carries() {
        assert!(draws_pretty_for(
            Some("truecolor"),
            Some("xterm-kitty"),
            None,
            &[]
        ));
        assert!(draws_pretty_for(
            Some("truecolor"),
            Some("xterm-256color"),
            Some("WezTerm"),
            &[]
        ));
        // A listed terminal is trusted for the depth as well as the glyphs: Windows
        // Terminal and Zed need not set `COLORTERM` to be known to draw 24-bit color.
        assert!(draws_pretty_for(None, Some("xterm-kitty"), None, &[]));
        assert!(draws_pretty_for(
            None,
            Some("xterm-256color"),
            Some("zed"),
            &[]
        ));
        assert!(draws_pretty_for(
            None,
            Some("xterm-256color"),
            None,
            &[true]
        ));
        assert!(!draws_pretty_for(
            Some("truecolor"),
            Some("xterm-256color"),
            Some("Apple_Terminal"),
            &[]
        ));
        assert!(!draws_pretty_for(None, Some("xterm-256color"), None, &[]));
    }
}
