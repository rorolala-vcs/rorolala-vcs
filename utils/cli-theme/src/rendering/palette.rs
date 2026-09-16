//! What a rendering pass is allowed to draw with, and how one run of text is drawn.

use colored::{ColoredString, Colorize};

use crate::colorize::is_enabled;
use crate::theme::{ThemeChoice, theme_choice};

/// A colour, as the numbers a terminal is handed for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Rgb {
    /// Red, from `0` to `255`.
    pub(super) red: u8,
    /// Green, from `0` to `255`.
    pub(super) green: u8,
    /// Blue, from `0` to `255`.
    pub(super) blue: u8,
}

impl Rgb {
    /// The colour `RRGGBB` names, if it names one.
    #[must_use]
    pub(super) fn from_hex(hex: &str) -> Option<Self> {
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        Some(Self {
            red: u8::from_str_radix(hex.get(0..2)?, 16).ok()?,
            green: u8::from_str_radix(hex.get(2..4)?, 16).ok()?,
            blue: u8::from_str_radix(hex.get(4..6)?, 16).ok()?,
        })
    }
}

/// The sixteen colours a terminal is expected to have.
///
/// The values are the ones a terminal falls back to when it has been told nothing
/// else, so that a colour named here and the same colour written out as `#RRGGBB` come
/// out the same.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NamedColor {
    /// Black.
    Black,
    /// Red.
    Red,
    /// Green.
    Green,
    /// Yellow.
    Yellow,
    /// Blue.
    Blue,
    /// Magenta.
    Magenta,
    /// Cyan.
    Cyan,
    /// White.
    White,
    /// Bright black, which is what a terminal draws grey as.
    BrightBlack,
    /// Bright red.
    BrightRed,
    /// Bright green.
    BrightGreen,
    /// Bright yellow.
    BrightYellow,
    /// Bright blue.
    BrightBlue,
    /// Bright magenta.
    BrightMagenta,
    /// Bright cyan.
    BrightCyan,
    /// Bright white.
    BrightWhite,
}

/// The colour each of the sixteen is drawn in by default.
const STANDARD: [Rgb; 16] = [
    Rgb {
        red: 0x00,
        green: 0x00,
        blue: 0x00,
    },
    Rgb {
        red: 0xcd,
        green: 0x00,
        blue: 0x00,
    },
    Rgb {
        red: 0x00,
        green: 0xcd,
        blue: 0x00,
    },
    Rgb {
        red: 0xcd,
        green: 0xcd,
        blue: 0x00,
    },
    Rgb {
        red: 0x00,
        green: 0x00,
        blue: 0xee,
    },
    Rgb {
        red: 0xcd,
        green: 0x00,
        blue: 0xcd,
    },
    Rgb {
        red: 0x00,
        green: 0xcd,
        blue: 0xcd,
    },
    Rgb {
        red: 0xe5,
        green: 0xe5,
        blue: 0xe5,
    },
    Rgb {
        red: 0x7f,
        green: 0x7f,
        blue: 0x7f,
    },
    Rgb {
        red: 0xff,
        green: 0x00,
        blue: 0x00,
    },
    Rgb {
        red: 0x00,
        green: 0xff,
        blue: 0x00,
    },
    Rgb {
        red: 0xff,
        green: 0xff,
        blue: 0x00,
    },
    Rgb {
        red: 0x5c,
        green: 0x5c,
        blue: 0xff,
    },
    Rgb {
        red: 0xff,
        green: 0x00,
        blue: 0xff,
    },
    Rgb {
        red: 0x00,
        green: 0xff,
        blue: 0xff,
    },
    Rgb {
        red: 0xff,
        green: 0xff,
        blue: 0xff,
    },
];

impl NamedColor {
    /// The colour `name` spells, where an upper-case name is the bright one.
    #[must_use]
    pub(super) fn by_name(name: &str) -> Option<Self> {
        let bright = name.chars().all(char::is_uppercase);
        let found = match name.to_ascii_lowercase().as_str() {
            "black" => Self::Black,
            "red" => Self::Red,
            "green" => Self::Green,
            "yellow" => Self::Yellow,
            "blue" => Self::Blue,
            "magenta" => Self::Magenta,
            "cyan" => Self::Cyan,
            "white" => Self::White,
            _ => return None,
        };
        Some(if bright { found.brightened() } else { found })
    }

    /// The same colour, bright.
    #[must_use]
    pub(super) const fn brightened(self) -> Self {
        match self {
            Self::Black => Self::BrightBlack,
            Self::Red => Self::BrightRed,
            Self::Green => Self::BrightGreen,
            Self::Yellow => Self::BrightYellow,
            Self::Blue => Self::BrightBlue,
            Self::Magenta => Self::BrightMagenta,
            Self::Cyan => Self::BrightCyan,
            Self::White
            | Self::BrightBlack
            | Self::BrightRed
            | Self::BrightGreen
            | Self::BrightYellow
            | Self::BrightBlue
            | Self::BrightMagenta
            | Self::BrightCyan
            | Self::BrightWhite => self,
        }
    }

    /// The colour this one is, as the terminal's palette spells it.
    // `as` rather than `usize::from`, because a `const fn` cannot call the conversion.
    #[allow(clippy::cast_lossless)]
    #[must_use]
    pub(super) const fn rgb(self) -> Rgb {
        STANDARD[self as usize]
    }

    /// The number a terminal addresses this colour by, from `0` to `15`.
    #[must_use]
    pub(super) const fn index(self) -> u8 {
        self as u8
    }

    /// The colour a palette number stands for, reading anything that is not one of
    /// the sixteen as black.
    #[must_use]
    pub(super) const fn from_index(index: u8) -> Self {
        match index {
            1 => Self::Red,
            2 => Self::Green,
            3 => Self::Yellow,
            4 => Self::Blue,
            5 => Self::Magenta,
            6 => Self::Cyan,
            7 => Self::White,
            8 => Self::BrightBlack,
            9 => Self::BrightRed,
            10 => Self::BrightGreen,
            11 => Self::BrightYellow,
            12 => Self::BrightBlue,
            13 => Self::BrightMagenta,
            14 => Self::BrightCyan,
            15 => Self::BrightWhite,
            _ => Self::Black,
        }
    }

    /// How `colored` names this colour.
    #[must_use]
    pub(super) const fn colored(self) -> colored::Color {
        match self {
            Self::Black => colored::Color::Black,
            Self::Red => colored::Color::Red,
            Self::Green => colored::Color::Green,
            Self::Yellow => colored::Color::Yellow,
            Self::Blue => colored::Color::Blue,
            Self::Magenta => colored::Color::Magenta,
            Self::Cyan => colored::Color::Cyan,
            Self::White => colored::Color::White,
            Self::BrightBlack => colored::Color::BrightBlack,
            Self::BrightRed => colored::Color::BrightRed,
            Self::BrightGreen => colored::Color::BrightGreen,
            Self::BrightYellow => colored::Color::BrightYellow,
            Self::BrightBlue => colored::Color::BrightBlue,
            Self::BrightMagenta => colored::Color::BrightMagenta,
            Self::BrightCyan => colored::Color::BrightCyan,
            Self::BrightWhite => colored::Color::BrightWhite,
        }
    }
}

/// The nearest of the sixteen to `color`.
///
/// Distance is the plain one between the two colours read as points, which is not how
/// an eye reads them, but is enough to keep a highlight recognisable once a theme has
/// only the sixteen to draw it in.
#[must_use]
pub(super) fn nearest(color: Rgb) -> NamedColor {
    STANDARD
        .iter()
        .enumerate()
        .min_by_key(|(_, candidate)| distance(color, **candidate))
        .map_or(NamedColor::Black, |(index, _)| {
            NamedColor::from_index(u8::try_from(index).unwrap_or(0))
        })
}

/// How far apart two colours are, as points.
fn distance(left: Rgb, right: Rgb) -> u32 {
    let channel = |one: u8, other: u8| {
        let delta = u32::from(one.abs_diff(other));
        delta * delta
    };
    channel(left.red, right.red) + channel(left.green, right.green) + channel(left.blue, right.blue)
}

/// A colour the language asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Color {
    /// One of the sixteen.
    Named(NamedColor),
    /// An exact colour, which only a theme that can ask for one gets.
    Exact(Rgb),
}

/// How a run of text is drawn.
// A style is four independent switches and the colours they are drawn over; there is
// no state machine in them to fold them into.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct StyleState<'a> {
    /// The colour of the text.
    pub(super) foreground: Option<Color>,
    /// The colour behind the text.
    pub(super) background: Option<Color>,
    /// Whether the text is bold.
    pub(super) bold: bool,
    /// Whether the text is italic.
    pub(super) italic: bool,
    /// Whether the text is underlined.
    pub(super) underline: bool,
    /// Whether the text is struck through.
    pub(super) strike: bool,
    /// The address the text opens, if it opens one.
    pub(super) link: Option<&'a str>,
}

impl StyleState<'_> {
    /// Nothing set.
    #[must_use]
    pub(super) const fn plain() -> Self {
        Self {
            foreground: None,
            background: None,
            bold: false,
            italic: false,
            underline: false,
            strike: false,
            link: None,
        }
    }

    /// The same style, drawn in `color`.
    #[must_use]
    pub(super) const fn foreground(mut self, color: Color) -> Self {
        self.foreground = Some(color);
        self
    }

    /// The same style, in bold.
    #[must_use]
    pub(super) const fn bolded(mut self) -> Self {
        self.bold = true;
        self
    }

    /// The style a quotation is drawn in.
    #[must_use]
    pub(super) const fn grey() -> Self {
        Self::plain().foreground(Color::Named(NamedColor::BrightBlack))
    }
}

/// The decisions a rendering pass is made with.
#[derive(Debug, Clone, Copy)]
pub(super) struct RenderOptions {
    /// How much the output may ask of the terminal.
    pub(super) theme: ThemeChoice,
    /// Whether the output is colored at all.
    pub(super) color: bool,
    /// Whether a link is written as the escape sequence a terminal can follow.
    ///
    /// Off wherever the result is measured rather than printed: the sequences for a
    /// link are not the `ESC [ ... m` ones a table can count, so a link inside a table
    /// would push every column out of line.
    pub(super) hyperlinks: bool,
    /// How many columns the terminal being drawn to has, if it can be asked.
    ///
    /// Asked and not assumed: a program whose output is going into a file has no width,
    /// and drawing a rule to the eighty columns it might have had is worse than drawing
    /// it to the message itself.
    pub(super) terminal: Option<usize>,
}

impl RenderOptions {
    /// What the program is drawn with right now.
    pub(super) fn current() -> Self {
        Self {
            theme: theme_choice(),
            color: is_enabled(),
            hyperlinks: true,
            terminal: terminal_width(),
        }
    }

    /// Whether colours are drawn at all.
    ///
    /// [`ThemeChoice::Text`] asks for the content of [`ThemeChoice::Simple`] in one
    /// flat colour, and the one colour a terminal already draws in is its default, so a
    /// flat theme is a theme with no colour rather than a second palette.
    ///
    /// [`ThemeChoice::Simple`]: crate::ThemeChoice::Simple
    fn colored(self) -> bool {
        self.color && self.theme != ThemeChoice::Text
    }
}

/// How many columns the terminal standard output is attached to has.
///
/// The same question `bat` asks of the same library, and the same answer: `None` where
/// there is no terminal to ask, which is the case that has to be handled rather than
/// papered over with a number nobody chose.
fn terminal_width() -> Option<usize> {
    console::Term::stdout()
        .size_checked()
        .map(|(_, columns)| usize::from(columns))
}

/// Draws `text` in `style`, as far as `options` lets it.
#[must_use]
pub(super) fn paint(text: &str, style: &StyleState<'_>, options: RenderOptions) -> String {
    if !options.colored() {
        return text.to_owned();
    }

    let mut painted = ColoredString::from(text);
    if let Some(foreground) = style.foreground {
        painted = painted.color(colored_color(foreground, options.theme));
    }
    if let Some(background) = style.background {
        painted = painted.on_color(colored_color(background, options.theme));
    }
    if style.bold {
        painted = painted.bold();
    }
    if style.italic {
        painted = painted.italic();
    }
    if style.underline {
        painted = painted.underline();
    }
    if style.strike {
        painted = painted.strikethrough();
    }
    let painted = painted.to_string();

    match style.link {
        Some(address) if options.hyperlinks => {
            format!("\u{1b}]8;;{address}\u{1b}\\{painted}\u{1b}]8;;\u{1b}\\")
        }
        _ => painted,
    }
}

/// How `color` is asked of the terminal, given what `theme` allows.
fn colored_color(color: Color, theme: ThemeChoice) -> colored::Color {
    match color {
        Color::Named(named) => named_color(named, theme),
        Color::Exact(rgb) => {
            if theme == ThemeChoice::Pretty {
                exact(rgb)
            } else {
                named_color(nearest(rgb), theme)
            }
        }
    }
}

/// How one of the sixteen is asked of the terminal, given what `theme` allows.
fn named_color(named: NamedColor, theme: ThemeChoice) -> colored::Color {
    if theme == ThemeChoice::Pretty {
        exact(named.rgb())
    } else {
        named.colored()
    }
}

/// The twenty-four bit colour `rgb` asks for.
const fn exact(rgb: Rgb) -> colored::Color {
    colored::Color::TrueColor {
        r: rgb.red,
        g: rgb.green,
        b: rgb.blue,
    }
}
