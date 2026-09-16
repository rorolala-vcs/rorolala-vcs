//! The inline half of the color language: the styles inside one line.

use super::palette::{Color, NamedColor, RenderOptions, Rgb, StyleState, paint};

/// The delimiters that turn an attribute on and off, longest first.
///
/// Longest first, so that `***` is read as one delimiter rather than as `**` followed
/// by `*`.
const DELIMITERS: [&str; 5] = ["***", "**", "__", "~~", "*"];

/// The characters a backslash takes the meaning away from.
const ESCAPABLE: [char; 4] = ['*', '_', '~', '\\'];

/// Renders the styles `text` asks for, starting from `base`.
///
/// `base` is the style the line is drawn in before it is read — grey inside a
/// quotation, the alert's colour inside an alert — so that what the line asks for
/// overrides it rather than nesting inside it.
#[must_use]
pub(super) fn render<'a>(text: &'a str, base: StyleState<'a>, options: RenderOptions) -> String {
    let mut parser = Parser {
        input: text,
        position: 0,
        options,
        output: String::new(),
        literal: String::new(),
        state: base,
        open: Vec::new(),
    };
    parser.run();
    parser.output
}

/// A pair of markers that draw what they hold in a style of its own.
///
/// Unlike the emphasis delimiters, these are kept: a code span is written with its
/// backticks on screen, which is what tells the reader that what is between them is
/// meant to be taken literally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wrapper {
    /// `` ` `` — a span of literal text.
    Code,
    /// `<` … `>` — a flag.
    Flag,
}

impl Wrapper {
    /// The wrapper `character` opens, if it opens one.
    const fn of(character: char) -> Option<Self> {
        match character {
            '`' => Some(Self::Code),
            '<' => Some(Self::Flag),
            _ => None,
        }
    }

    /// The characters this one is written between.
    const fn markers(self) -> (char, char) {
        match self {
            Self::Code => ('`', '`'),
            Self::Flag => ('<', '>'),
        }
    }

    /// `state` as this wrapper draws it.
    #[must_use]
    const fn style(self, mut state: StyleState<'_>) -> StyleState<'_> {
        state.foreground = Some(Color::Named(NamedColor::BrightCyan));
        match self {
            Self::Code => state.bold = true,
            Self::Flag => state.italic = true,
        }
        state
    }
}

/// What a delimiter turns on.
// A style is four independent switches; there is no state machine in them to fold them
// into, and `StyleState` is where they are put together.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default)]
struct Attributes {
    /// Bold.
    bold: bool,
    /// Italic.
    italic: bool,
    /// Underline.
    underline: bool,
    /// Strike-through.
    strike: bool,
}

impl Attributes {
    /// The attributes `delimiter` turns on.
    #[must_use]
    fn of(delimiter: &str) -> Self {
        match delimiter {
            "***" => Self {
                bold: true,
                italic: true,
                ..Self::default()
            },
            "**" => Self {
                bold: true,
                ..Self::default()
            },
            "__" => Self {
                underline: true,
                ..Self::default()
            },
            "~~" => Self {
                strike: true,
                ..Self::default()
            },
            _ => Self {
                italic: true,
                ..Self::default()
            },
        }
    }

    /// Whether everything this turns on is already on.
    #[must_use]
    const fn active(self, state: StyleState<'_>) -> bool {
        (!self.bold || state.bold)
            && (!self.italic || state.italic)
            && (!self.underline || state.underline)
            && (!self.strike || state.strike)
    }

    /// `state` with everything this turns on turned on.
    #[must_use]
    const fn on(self, mut state: StyleState<'_>) -> StyleState<'_> {
        state.bold |= self.bold;
        state.italic |= self.italic;
        state.underline |= self.underline;
        state.strike |= self.strike;
        state
    }

    /// `state` with everything this turns on turned off.
    #[must_use]
    const fn off(self, mut state: StyleState<'_>) -> StyleState<'_> {
        state.bold &= !self.bold;
        state.italic &= !self.italic;
        state.underline &= !self.underline;
        state.strike &= !self.strike;
        state
    }
}

/// What a `[[...]]` directive does.
enum Directive<'a> {
    /// Draw in the style the innermost directive was written in.
    Close,
    /// Draw in `style` until the matching `[[/]]`.
    Open(StyleState<'a>),
}

/// A style the line has moved into, and what it has to be closed by.
#[derive(Debug, Clone, Copy)]
struct Open<'a> {
    /// What opened it.
    by: Opened,
    /// The style to come back to.
    state: StyleState<'a>,
}

/// What a style was opened by, which is also what closes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opened {
    /// A `[[...]]` directive, closed by `[[/]]`.
    Directive,
    /// A pair of markers, closed by the second of the two.
    Wrapper(Wrapper),
}

/// One pass over one line.
struct Parser<'a> {
    /// The line being read.
    input: &'a str,
    /// How far into it the pass has got.
    position: usize,
    /// What the pass may draw with.
    options: RenderOptions,
    /// What has been drawn so far.
    output: String,
    /// Text read but not yet drawn, because the style it is drawn in may still change.
    literal: String,
    /// The style text is currently drawn in.
    state: StyleState<'a>,
    /// The styles the line has moved into, innermost last.
    open: Vec<Open<'a>>,
}

impl<'a> Parser<'a> {
    /// Reads the line.
    fn run(&mut self) {
        let input = self.input;
        while self.position < input.len() {
            let rest = &input[self.position..];
            let Some(character) = rest.chars().next() else {
                break;
            };
            if rest.starts_with('\\') {
                self.escape(rest);
            } else if rest.starts_with("[[") {
                self.directive(rest);
            } else if let Some(wrapper) = self.wrapper(character, rest) {
                self.mark(wrapper, character);
            } else if let Some(delimiter) =
                DELIMITERS.iter().find(|found| rest.starts_with(**found))
            {
                self.delimiter(rest, delimiter);
            } else {
                self.advance();
            }
        }
        self.flush();
    }

    /// Reads one character as itself.
    fn advance(&mut self) {
        let Some(character) = self.input[self.position..].chars().next() else {
            return;
        };
        self.literal.push(character);
        self.position += character.len_utf8();
    }

    /// Reads a backslash: a character written as itself, or a character named by
    /// number.
    fn escape(&mut self, rest: &'a str) {
        if let Some((character, consumed)) = unicode_escape(rest) {
            self.literal.push(character);
            self.position += consumed;
            return;
        }
        let mut characters = rest.chars();
        characters.next();
        match characters.next() {
            Some(escaped) if ESCAPABLE.contains(&escaped) => {
                self.literal.push(escaped);
                self.position += 1 + escaped.len_utf8();
            }
            // Not an escape after all: the backslash is a backslash, and whatever
            // follows it is read on its own merits.
            _ => {
                self.literal.push('\\');
                self.position += 1;
            }
        }
    }

    /// The wrapper `character` opens or closes, if it marks one.
    ///
    /// A marker closes where the wrapper it belongs to is already open, and opens only
    /// where the marker that would close it is still to come. Without that much
    /// lookahead, `a < b` would open a flag that nobody meant and never close it.
    fn wrapper(&self, character: char, rest: &str) -> Option<Wrapper> {
        if let Some(Opened::Wrapper(wrapper)) = self.open.last().map(|open| open.by)
            && wrapper.markers().1 == character
        {
            return Some(wrapper);
        }
        let wrapper = Wrapper::of(character)?;
        let after = &rest[character.len_utf8()..];
        after.contains(wrapper.markers().1).then_some(wrapper)
    }

    /// Reads one of the two markers of `wrapper`.
    ///
    /// What is held is drawn in the wrapper's style, markers and all, so whichever
    /// marker this is goes into the text as it is read.
    fn mark(&mut self, wrapper: Wrapper, character: char) {
        if self
            .open
            .last()
            .is_some_and(|open| open.by == Opened::Wrapper(wrapper))
        {
            // The closing marker is drawn with what it closes.
            self.literal.push(character);
            self.flush();
            self.close(Opened::Wrapper(wrapper));
        } else {
            self.flush();
            self.open.push(Open {
                by: Opened::Wrapper(wrapper),
                state: self.state,
            });
            self.state = wrapper.style(self.state);
            // The opening marker is drawn with what it opens.
            self.literal.push(character);
        }
        self.position += character.len_utf8();
    }

    /// Reads a `[[...]]` directive.
    ///
    /// A `[[` that opens nothing is left in the text, so that a message can still say
    /// what it means when it means to say `[[`.
    fn directive(&mut self, rest: &'a str) {
        let Some(end) = rest.find("]]") else {
            self.literal.push_str("[[");
            self.position += 2;
            return;
        };
        let Some(directive) = self.resolve(&rest[2..end]) else {
            self.literal.push_str("[[");
            self.position += 2;
            return;
        };

        self.flush();
        match directive {
            Directive::Close => self.close(Opened::Directive),
            Directive::Open(state) => {
                self.open.push(Open {
                    by: Opened::Directive,
                    state: self.state,
                });
                self.state = state;
            }
        }
        self.position += end + 2;
    }

    /// What `inner` asks for, if it asks for anything.
    fn resolve(&self, inner: &'a str) -> Option<Directive<'a>> {
        if inner == "/" {
            return Some(Directive::Close);
        }
        if let Some(address) = inner.strip_prefix('?') {
            let mut state = self.state;
            state.link = Some(address);
            return Some(Directive::Open(state));
        }
        if let Some(name) = inner.strip_prefix('~') {
            return color(name).map(|color| {
                let mut state = self.state;
                state.background = Some(color);
                Directive::Open(state)
            });
        }
        color(inner).map(|color| {
            let mut state = self.state;
            state.foreground = Some(color);
            Directive::Open(state)
        })
    }

    /// Reads a delimiter that turns attributes on or off.
    ///
    /// A delimiter opens only when the same one closes later on the line. Without that
    /// much lookahead, `2 * 3` would italicise its way to the end of the message, and
    /// the point of a style language is that it can be written in passing.
    fn delimiter(&mut self, rest: &'a str, delimiter: &str) {
        let attributes = Attributes::of(delimiter);
        let after = &rest[delimiter.len()..];
        if attributes.active(self.state) {
            self.flush();
            self.state = attributes.off(self.state);
        } else if after.contains(delimiter) {
            self.flush();
            self.state = attributes.on(self.state);
        } else {
            self.literal.push_str(delimiter);
        }
        self.position += delimiter.len();
    }

    /// Comes back to the style the innermost thing opened by `opened` was written in.
    ///
    /// Whatever was opened after it goes back with it, so that a directive and a pair of
    /// markers can hold one another without closing each other's work.
    fn close(&mut self, opened: Opened) {
        let Some(at) = self.open.iter().rposition(|open| open.by == opened) else {
            return;
        };
        let state = self.open[at].state;
        self.open.truncate(at);
        self.state = state;
    }

    /// Draws what has been read, in the style it was read in.
    fn flush(&mut self) {
        if self.literal.is_empty() {
            return;
        }
        let drawn = paint(&self.literal, &self.state, self.options);
        self.output.push_str(&drawn);
        self.literal.clear();
    }
}

/// The colour `text` names, by name or by number.
fn color(text: &str) -> Option<Color> {
    if let Some(hex) = text.strip_prefix('#') {
        return Rgb::from_hex(hex).map(Color::Exact);
    }
    NamedColor::by_name(text).map(Color::Named)
}

/// The character `\u...` names, and how many bytes of `rest` it took up.
///
/// Both spellings a person reaches for are taken: `\u2764`, which is four hex digits,
/// and `\u{2764}`, which is as many as the character needs.
///
/// A character outside the Basic Multilingual Plane comes out of anything that speaks
/// UTF-16 as a pair — `\udb82\udce3` is one `NerdFont` glyph, not two — so a high
/// surrogate is read together with the low surrogate that has to follow it. Half of a
/// pair on its own is not a character, and is left as it is written.
fn unicode_escape(rest: &str) -> Option<(char, usize)> {
    let (unit, consumed) = code_unit(rest)?;
    if matches!(unit, 0xd800..=0xdbff) {
        let (low, following) = code_unit(rest.get(consumed..)?)?;
        if !matches!(low, 0xdc00..=0xdfff) {
            return None;
        }
        let joined = 0x10000 + ((unit - 0xd800) << 10) + (low - 0xdc00);
        return Some((char::from_u32(joined)?, consumed + following));
    }
    Some((char::from_u32(unit)?, consumed))
}

/// One `\u...` escape: the code unit it names, which a pair makes a character out of,
/// and how many bytes of `rest` it took up.
fn code_unit(rest: &str) -> Option<(u32, usize)> {
    let after = rest.strip_prefix("\\u")?;
    let (digits, consumed) = if let Some(braced) = after.strip_prefix('{') {
        let end = braced.find('}')?;
        (&braced[..end], end + 4)
    } else {
        if after.len() < 4 || !after.is_char_boundary(4) {
            return None;
        }
        (&after[..4], 6)
    };
    Some((u32::from_str_radix(digits, 16).ok()?, consumed))
}
