//! The color language Rorolala's messages are written in.
//!
//! A message is written the way it should read, with the styles marked in it, and is
//! drawn once by [`TextRendering::render`] — or, more briefly, by `trd!`. What the
//! drawing may use is not decided here: it is read off the program's
//! [`ThemeChoice`] and coloring switch, so one source draws itself as plain text on a
//! terminal that wants none of it and as glyphs in true color on one that wants both.
//!
//! The blocks are the ones a person already knows from Markdown — headings, quotes,
//! alerts, rules, pipe tables and fenced code — and the inline styles are a smaller,
//! deliberately non-Markdown set, because a message is not a document: a `[[red]]`
//! stays on until its `[[/]]`, which nests, where Markdown's emphasis only ever pairs
//! up.
//!
//! [`ThemeChoice`]: crate::ThemeChoice

mod ansi;
mod code;
mod inline;
mod palette;
mod table;

use crate::ThemeChoice;

use self::ansi::display_width;
use self::palette::{Color, NamedColor, RenderOptions, StyleState, paint};

/// Draws the color language.
///
/// The only entry point a caller needs: hand it the message, and it comes back with
/// the escapes that draw it, or with none of them, if that is what the program was
/// told to want.
///
/// ```rust
/// use rorolala_utils_cli_theme::TextRendering;
///
/// assert_eq!(TextRendering::render("# Title\n\nBody."), "Title\n\nBody.");
/// ```
#[derive(Debug, Clone, Copy)]
pub struct TextRendering;

impl TextRendering {
    /// Renders `source` for the theme and switch the program is currently drawn with.
    #[must_use]
    pub fn render(source: &str) -> String {
        render_with(source, RenderOptions::current())
    }
}

/// Draws the color language, for the options passed rather than the program's.
fn render_with(source: &str, options: RenderOptions) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut output: Vec<Line> = Vec::new();
    let mut owed_blank = false;
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];

        if line.trim().is_empty() {
            output.push(Line::Text(String::new()));
            owed_blank = false;
            index += 1;
            continue;
        }

        if let Some(fence) = Fence::opening(line) {
            settled(&mut output, &mut owed_blank);
            let (block, next) = fence.take(&lines, index + 1);
            push_text(
                &mut output,
                &code::highlight(&block, fence.language.as_deref(), options),
            );
            index = next;
            continue;
        }

        if table::starts(&lines, index) {
            settled(&mut output, &mut owed_blank);
            let (rows, next) = table::render(&lines, index, options);
            for row in rows {
                output.push(Line::Text(row));
            }
            index = next;
            continue;
        }

        if is_rule(line) {
            settled(&mut output, &mut owed_blank);
            output.push(Line::Rule);
            index += 1;
            continue;
        }

        if let Some((level, text)) = heading(line) {
            settled(&mut output, &mut owed_blank);
            blank(&mut output);
            let style = if level <= 3 {
                StyleState::plain().bolded()
            } else {
                StyleState::plain()
            };
            output.push(Line::Text(inline::render(text, style, options)));
            owed_blank = true;
            index += 1;
            continue;
        }

        if line.starts_with('>') {
            settled(&mut output, &mut owed_blank);
            let (body, next) = quote(&lines, index);
            push_text(&mut output, &drawn_quote(&body, options));
            index = next;
            continue;
        }

        settled(&mut output, &mut owed_blank);
        output.push(Line::Text(inline::render(
            line,
            StyleState::plain(),
            options,
        )));
        index += 1;
    }

    finish(output, options)
}

/// One line of a rendered document.
enum Line {
    /// Text that is already drawn.
    Text(String),
    /// A rule, drawn as wide as the document once its width is known.
    Rule,
}

/// Adds an empty line, unless the last line already is one.
fn blank(output: &mut Vec<Line>) {
    if !output.last().is_some_and(is_blank_line) {
        output.push(Line::Text(String::new()));
    }
}

/// Whether `line` is one of the document's empty lines.
const fn is_blank_line(line: &Line) -> bool {
    matches!(line, Line::Text(text) if text.is_empty())
}

/// Pays the blank line a heading asked for, and says none is owed any more.
fn settled(output: &mut Vec<Line>, owed_blank: &mut bool) {
    if *owed_blank {
        blank(output);
        *owed_blank = false;
    }
}

/// Adds `text`, which may be several lines, one line at a time.
fn push_text(output: &mut Vec<Line>, text: &str) {
    for line in text.lines() {
        output.push(Line::Text(line.to_owned()));
    }
}

/// Draws the rules, trims what is drawn, and drops the blank lines around it.
fn finish(lines: Vec<Line>, options: RenderOptions) -> String {
    let width = rule_width(&lines, options);
    let mark = if options.theme == ThemeChoice::Pretty {
        '\u{2500}'
    } else {
        '-'
    };
    let rule = paint(
        &mark.to_string().repeat(width),
        &StyleState::grey(),
        options,
    );
    let drawn: Vec<String> = lines
        .into_iter()
        .map(|line| match line {
            Line::Text(text) => text.trim_end().to_owned(),
            Line::Rule => rule.clone(),
        })
        .collect();

    let start = drawn
        .iter()
        .position(|line| !line.is_empty())
        .unwrap_or(drawn.len());
    let end = drawn
        .iter()
        .rposition(|line| !line.is_empty())
        .map_or(start, |last| last + 1);
    drawn[start..end].join("\n")
}

/// How wide a rule is drawn, in columns.
///
/// A theme that carries the box-drawing characters draws it across the terminal, made
/// of the same character a table's lines are made of, because that is the line the
/// reader's eye is already following. Every other theme draws it across the message
/// instead, where a `-` beyond the text it separates is just a run of dashes — and
/// falls back to the message even under the glyphs where the terminal cannot be asked
/// how wide it is.
fn rule_width(lines: &[Line], options: RenderOptions) -> usize {
    if options.theme == ThemeChoice::Pretty
        && let Some(terminal) = options.terminal
    {
        return terminal.max(1);
    }
    content_width(lines).max(1)
}

/// The width the widest line of the message is drawn to.
fn content_width(lines: &[Line]) -> usize {
    lines
        .iter()
        .filter_map(|line| match line {
            Line::Text(text) => Some(display_width(text)),
            Line::Rule => None,
        })
        .max()
        .unwrap_or(0)
}

/// Whether `line` asks for a rule across the document.
///
/// More than three `-` and nothing else: three are a table's separator row and one or
/// two are ordinary text, so the threshold is what keeps a rule from being drawn
/// where nobody asked for one.
fn is_rule(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() > 3 && trimmed.bytes().all(|byte| byte == b'-')
}

/// The level and text of the heading `line` is, if it is one.
///
/// The `#` has to open the line, there has to be a space after it, and something has to
/// follow that space: `#` alone is a comment, `#No` is a tag, and neither is a title.
fn heading(line: &str) -> Option<(usize, &str)> {
    let level = line.bytes().take_while(|byte| *byte == b'#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let text = line.get(level..)?.strip_prefix(' ')?.trim();
    if text.is_empty() {
        return None;
    }
    Some((level, text))
}

/// The lines the quote starting at `index` is made of, without their `>` marker, and
/// where the next block begins.
fn quote<'a>(lines: &'a [&'a str], index: usize) -> (Vec<&'a str>, usize) {
    let mut body = Vec::new();
    let mut next = index;
    while next < lines.len() {
        let Some(rest) = lines[next].strip_prefix('>') else {
            break;
        };
        body.push(rest.strip_prefix(' ').unwrap_or(rest));
        next += 1;
    }
    (body, next)
}

/// Draws a quoted block, which is an alert when its first line declares one.
fn drawn_quote(body: &[&str], options: RenderOptions) -> String {
    body.first().copied().and_then(alert).map_or_else(
        || {
            let style = StyleState::grey();
            body.iter()
                .map(|line| quoted(line, style, options))
                .collect::<Vec<_>>()
                .join("\n")
        },
        |kind| {
            let style = StyleState::plain().foreground(Color::Named(kind.color()));
            drawn_alert(kind, &body[1..], style, options)
        },
    )
}

/// Draws an alert: a named header, then the warning itself.
fn drawn_alert(
    kind: AlertKind,
    body: &[&str],
    style: StyleState<'_>,
    options: RenderOptions,
) -> String {
    let mut drawn = quoted(&alert_title(kind, options), style.bolded(), options);
    for line in body {
        drawn.push('\n');
        drawn.push_str(&quoted(line, style, options));
    }
    drawn
}

/// The header an alert is drawn under.
///
/// Only a theme that carries the glyphs can show which alert this is by its mark, so
/// every other theme says the name out loud instead.
fn alert_title(kind: AlertKind, options: RenderOptions) -> String {
    if options.theme == ThemeChoice::Pretty {
        format!("{} {}", kind.glyph(), kind.title())
    } else {
        format!("{}:", kind.name())
    }
}

/// One line of a block drawn beside a `|` gutter.
///
/// The gutter is drawn in `style` and the line is drawn in it too, so that a style
/// inside the line — a `[[red]]`, say — overrides it rather than nesting in it.
fn quoted(line: &str, style: StyleState<'_>, options: RenderOptions) -> String {
    if line.is_empty() {
        return paint("|", &style, options);
    }
    format!(
        "{}{}",
        paint("| ", &style, options),
        inline::render(line, style, options)
    )
}

/// The alert `line` declares, as in `[!Warning]`.
fn alert(line: &str) -> Option<AlertKind> {
    let declared = line.trim().strip_prefix("[!")?.strip_suffix(']')?;
    AlertKind::by_name(declared)
}

/// The kinds of alert the language knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlertKind {
    /// Something that has to be read before anything else.
    Important,
    /// Something that saves the reader a step.
    Tip,
    /// Something worth knowing.
    Note,
    /// Something that will go wrong.
    Warning,
    /// Something that has already gone wrong.
    Caution,
}

impl AlertKind {
    /// The kind `declared` names, whatever its case.
    fn by_name(declared: &str) -> Option<Self> {
        match declared.trim().to_ascii_lowercase().as_str() {
            "important" => Some(Self::Important),
            "tip" => Some(Self::Tip),
            "note" => Some(Self::Note),
            "warning" => Some(Self::Warning),
            "caution" => Some(Self::Caution),
            _ => None,
        }
    }

    /// The name this kind is headed with.
    ///
    /// Upper case, because that is what stands in for the glyph when a theme has none.
    const fn name(self) -> &'static str {
        match self {
            Self::Important => "IMPORTANT",
            Self::Tip => "TIP",
            Self::Note => "NOTE",
            Self::Warning => "WARNING",
            Self::Caution => "CAUTION",
        }
    }

    /// The name this kind is headed with, where a glyph already marks it.
    const fn title(self) -> &'static str {
        match self {
            Self::Important => "Important",
            Self::Tip => "Tip",
            Self::Note => "Note",
            Self::Warning => "Warning",
            Self::Caution => "Caution",
        }
    }

    /// The `NerdFont` glyph this kind is marked with.
    const fn glyph(self) -> char {
        match self {
            Self::Important => '\u{f03eb}',
            Self::Tip => '\u{f0335}',
            Self::Note => '\u{f00ba}',
            Self::Warning => '\u{f071}',
            Self::Caution => '\u{ea6c}',
        }
    }

    /// The colour this kind is drawn in.
    const fn color(self) -> NamedColor {
        match self {
            Self::Important => NamedColor::Magenta,
            Self::Tip => NamedColor::BrightGreen,
            Self::Note => NamedColor::BrightCyan,
            Self::Warning => NamedColor::BrightYellow,
            Self::Caution => NamedColor::BrightRed,
        }
    }
}

/// A fenced block of code.
struct Fence {
    /// The character the fence is made of, so the right one closes it.
    marker: char,
    /// How many of them the opening fence was made of.
    length: usize,
    /// The language the info string names, if it names one.
    language: Option<String>,
}

impl Fence {
    /// The fence `line` opens, if it opens one.
    fn opening(line: &str) -> Option<Self> {
        let marker = line.chars().next()?;
        if marker != '`' && marker != '~' {
            return None;
        }
        let length = line.chars().take_while(|found| *found == marker).count();
        if length < 3 {
            return None;
        }
        let info = line[length..].trim();
        Some(Self {
            marker,
            length,
            language: info.split_whitespace().next().map(str::to_owned),
        })
    }

    /// The code the fence starting at `index` holds, and where the next block begins.
    fn take(&self, lines: &[&str], index: usize) -> (String, usize) {
        let mut code = String::new();
        let mut next = index;
        while next < lines.len() {
            if self.closes(lines[next]) {
                next += 1;
                break;
            }
            if !code.is_empty() {
                code.push('\n');
            }
            code.push_str(lines[next]);
            next += 1;
        }
        (code, next)
    }

    /// Whether `line` closes this fence.
    fn closes(&self, line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.chars().count() >= self.length && trimmed.chars().all(|found| found == self.marker)
    }
}

/// Renders the color language.
///
/// The same call as [`TextRendering::render`], for the places where a macro reads
/// better than a path — which is everywhere a message is written.
///
/// A message written out in the source is read by [`format!`], so a value can be put
/// into it by position or by name, and a message the program is already holding is
/// rendered as it stands:
///
/// ```rust
/// use rorolala_utils_cli_theme::{set_enabled, trd};
///
/// set_enabled(false);
/// assert_eq!(trd!("Hello, **world**!"), "Hello, world!");
/// assert_eq!(trd!("Hello, **{}**!", "world"), "Hello, world!");
///
/// let name = "world";
/// assert_eq!(trd!("Hello, **{name}**!"), "Hello, world!");
///
/// let held = String::from("Hello, **world**!");
/// assert_eq!(trd!(&held), "Hello, world!");
/// ```
#[macro_export]
macro_rules! trd {
    // A message written out, which `format!` reads whether or not it turns out to hold
    // anything to fill in. This rule comes first for that reason: a lone literal matches
    // the rule below as well, and it is the one that has to lose.
    ($format:literal $(, $argument:tt)*) => {
        $crate::TextRendering::render(&::std::format!($format $(, $argument)*))
    };
    // A message the program already holds.
    ($text:expr) => {
        $crate::TextRendering::render(&$text)
    };
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::ThemeChoice;

    use super::{RenderOptions, render_with};

    /// The options the tests draw with.
    ///
    /// Colour is off, so that a test compares the language rather than the escapes
    /// drawn around it. The theme is still passed, because the theme is what decides
    /// the shape of an alert rather than only its colouring.
    fn options(theme: ThemeChoice) -> RenderOptions {
        RenderOptions {
            theme,
            color: false,
            hyperlinks: false,
            terminal: None,
        }
    }

    /// `source` drawn for a terminal with nothing special to offer.
    fn drawn(source: &str) -> String {
        render_with(source, options(ThemeChoice::Simple))
    }

    /// The colouring switch is one for the whole process, so the tests that turn it on
    /// take turns rather than race each other for it.
    static SWITCH: Mutex<()> = Mutex::new(());

    /// Draws what `body` asks for with colouring on.
    fn with_color<T>(body: impl FnOnce() -> T) -> T {
        let _held = SWITCH
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::set_enabled(true);
        let value = body();
        crate::unset_enabled();
        value
    }

    /// The options a test that wants escapes draws with.
    fn colored(theme: ThemeChoice) -> RenderOptions {
        RenderOptions {
            color: true,
            ..options(theme)
        }
    }

    #[test]
    // The braces in these messages are meant literally: what is being checked is that a
    // message the program holds is not read as a format string.
    #[allow(clippy::literal_string_with_formatting_args)]
    fn a_message_can_be_written_as_a_format_string() {
        assert_eq!(crate::trd!("Hello, **{}**!", "world"), "Hello, world!");

        let name = "world";
        assert_eq!(crate::trd!("Hello, **{name}**!"), "Hello, world!");

        // A message the program holds is rendered as it stands, and is not read as a
        // format string even where it holds braces.
        let held = String::from("Hello, {name}!");
        assert_eq!(crate::trd!(&held), "Hello, {name}!");
        assert_eq!(crate::trd!(held), "Hello, {name}!");
    }

    #[test]
    fn a_message_is_trimmed() {
        assert_eq!(drawn("\n\n  a message  \n\n"), "  a message");
    }

    #[test]
    fn a_code_span_and_a_flag_keep_their_markers() {
        assert_eq!(drawn("`code`"), "`code`");
        assert_eq!(drawn("<flag>"), "<flag>");
        // A marker with nothing to pair with is a character like any other.
        assert_eq!(drawn("a ` b"), "a ` b");
        assert_eq!(drawn("a < b"), "a < b");
    }

    #[test]
    fn a_code_span_and_a_flag_are_drawn_in_bright_cyan() {
        // Bold for what is meant to be taken literally, italic for a flag, and the
        // same bright cyan for both.
        let (code, flag) = with_color(|| {
            (
                render_with("`code`", colored(ThemeChoice::Simple)),
                render_with("<flag>", colored(ThemeChoice::Simple)),
            )
        });

        assert_eq!(code, "\u{1b}[1;96m`code`\u{1b}[0m");
        assert_eq!(flag, "\u{1b}[3;96m<flag>\u{1b}[0m");
    }

    #[test]
    fn a_code_span_can_hold_a_directive() {
        assert_eq!(drawn("`a [[red]]b[[/]] c`"), "`a b c`");

        // The directive takes the style it was written in and changes one thing about
        // it — the code span is still bold, so the `b` is bold and red — and the code
        // span carries on in the style it was written in rather than the one the
        // directive left behind.
        let drawn = with_color(|| render_with("`a [[red]]b[[/]] c`", colored(ThemeChoice::Simple)));

        assert_eq!(
            drawn,
            "\u{1b}[1;96m`a \u{1b}[0m\u{1b}[1;31mb\u{1b}[0m\u{1b}[1;96m c`\u{1b}[0m"
        );
    }

    #[test]
    fn inline_styles_lose_only_their_markers() {
        assert_eq!(drawn("Hello, **world**!"), "Hello, world!");
        assert_eq!(drawn("[[red]]red[[/]] and plain"), "red and plain");
        assert_eq!(drawn("a ***both*** b"), "a both b");
    }

    #[test]
    fn a_directive_nests() {
        assert_eq!(drawn("[[red]]a [[blue]]b[[/]] c[[/]]"), "a b c");
    }

    #[test]
    fn an_unmatched_delimiter_is_text() {
        assert_eq!(drawn("2 * 3"), "2 * 3");
        assert_eq!(drawn("100% **"), "100% **");
    }

    #[test]
    fn what_a_delimiter_pair_holds_is_between_it() {
        assert_eq!(drawn("a * b * c"), "a  b  c");
    }

    #[test]
    fn a_backslash_takes_the_meaning_away() {
        assert_eq!(drawn(r"\*not italic\*"), "*not italic*");
        assert_eq!(drawn(r"a\\b"), "a\\b");
    }

    #[test]
    fn a_character_can_be_named_by_number() {
        assert_eq!(drawn(r"\u2764"), "\u{2764}");
        assert_eq!(drawn(r"\u{2764}"), "\u{2764}");
        assert_eq!(drawn(r"\u{1f600}"), "\u{1f600}");
        assert_eq!(drawn(r"\uzzzz"), r"\uzzzz");
    }

    #[test]
    fn a_character_outside_the_plane_can_be_named_by_two() {
        // `\u{f08e3}`, an icon out of the Private Use Area, as UTF-16 spells it.
        assert_eq!(drawn(r"\udb82\udce3"), "\u{f08e3}");
        assert_eq!(drawn(r"\u{db82}\u{dce3}"), "\u{f08e3}");

        // Half of a pair is not a character, so it is left as it is written.
        assert_eq!(drawn(r"\udb82"), r"\udb82");
        assert_eq!(drawn(r"\udce3"), r"\udce3");
        assert_eq!(drawn(r"\udb82\udb82"), r"\udb82\udb82");
        assert_eq!(drawn(r"\udb82abc"), r"\udb82abc");
    }

    #[test]
    fn a_heading_is_a_title_with_a_line_either_side() {
        assert_eq!(drawn("text\n# Title\ntext"), "text\n\nTitle\n\ntext");
        assert_eq!(drawn("# Title\n\nText"), "Title\n\nText");
        assert_eq!(
            drawn("#### Not bold, still spaced"),
            "Not bold, still spaced"
        );
    }

    #[test]
    fn a_hash_with_nothing_after_it_is_not_a_heading() {
        assert_eq!(drawn("#no space"), "#no space");
        assert_eq!(drawn("####### too many"), "####### too many");
    }

    #[test]
    fn a_quotation_is_a_gutter_and_the_line() {
        assert_eq!(drawn("> quoted"), "| quoted");
        assert_eq!(drawn("> first\n>\n> third"), "| first\n|\n| third");
    }

    #[test]
    fn an_alert_says_its_name_where_the_glyphs_are_missing() {
        let warning = "> [!Warning]\n>\n> something went wrong";
        assert_eq!(drawn(warning), "| WARNING:\n|\n| something went wrong");
    }

    #[test]
    fn an_alert_is_marked_where_the_glyphs_are_there() {
        let warning = "> [!Warning]\n>\n> something went wrong";
        assert_eq!(
            render_with(warning, options(ThemeChoice::Pretty)),
            "| \u{f071} Warning\n|\n| something went wrong"
        );
    }

    #[test]
    fn an_alert_nobody_declared_is_a_quotation() {
        assert_eq!(drawn("> [!nonsense]\n> text"), "| [!nonsense]\n| text");
    }

    #[test]
    fn a_rule_is_as_wide_as_the_message() {
        assert_eq!(drawn("Hello\n----\nWorld"), "Hello\n-----\nWorld");
    }

    #[test]
    fn a_rule_reaches_the_terminal_where_the_glyphs_are_there() {
        let options = RenderOptions {
            terminal: Some(12),
            ..options(ThemeChoice::Pretty)
        };
        assert_eq!(
            render_with("Hello\n----\nWorld", options),
            "Hello\n────────────\nWorld"
        );
    }

    #[test]
    fn a_rule_stays_with_the_message_where_the_terminal_is_not_known() {
        assert_eq!(
            render_with("Hello\n----\nWorld", options(ThemeChoice::Pretty)),
            "Hello\n─────\nWorld"
        );
    }

    #[test]
    fn a_table_lined_up_where_its_cells_are_drawn_in_a_colour() {
        let source = "\
            | Name | State |\n\
            | --- | --- |\n\
            | a | [[GREEN]]up[[/]] |\n\
            | b | down |";

        let drawn = with_color(|| {
            [ThemeChoice::Simple, ThemeChoice::Pretty]
                .map(|theme| render_with(source, colored(theme)))
        });

        for (theme, drawn) in [ThemeChoice::Simple, ThemeChoice::Pretty]
            .into_iter()
            .zip(drawn)
        {
            // Every line is as wide as every other, escapes and all: `prettytable` counts
            // an escape sequence as a column, so a cell drawn in a colour is the one that
            // shows whether that was accounted for.
            let widths: Vec<usize> = drawn.lines().map(super::ansi::display_width).collect();
            assert!(
                widths.windows(2).all(|pair| pair[0] == pair[1]),
                "{theme:?}: {drawn:?}\n{widths:?}"
            );
            assert!(drawn.contains("\u{1b}["), "{theme:?}: {drawn:?}");
        }
    }

    #[test]
    fn a_table_is_drawn_with_its_columns_aligned() {
        let drawn = drawn("| Name | Value |\n| :--- | ---: |\n| a | 1 |");
        let lines: Vec<&str> = drawn.lines().collect();
        assert_eq!(lines.len(), 5, "{drawn}");
        assert!(lines[1].contains("| Name | Value |"), "{drawn}");
        assert!(lines[3].contains("| a    |     1 |"), "{drawn}");
    }

    #[test]
    fn a_table_is_drawn_in_the_other_alphabet_where_the_glyphs_are_there() {
        let source = "| Name | Value |\n| --- | --- |\n| a | 1 |";
        let drawn = render_with(source, options(ThemeChoice::Pretty));
        assert!(drawn.starts_with('\u{250c}'), "{drawn}");
        assert!(
            drawn.lines().any(|line| line.starts_with('\u{251c}')),
            "{drawn}"
        );
        assert!(drawn.ends_with('\u{2518}'), "{drawn}");
    }

    #[test]
    fn a_style_is_drawn_where_the_terminal_carries_one() {
        let (simple, pretty) = with_color(|| {
            (
                render_with("[[red]]red[[/]]", colored(ThemeChoice::Simple)),
                render_with("[[#ff0000]]red[[/]]", colored(ThemeChoice::Pretty)),
            )
        });

        assert!(simple.contains("\u{1b}[31mred"), "{simple:?}");
        assert!(pretty.contains("\u{1b}[38;2;255;0;0mred"), "{pretty:?}");
    }

    #[test]
    fn a_flat_theme_draws_no_escapes_at_all() {
        // Colouring is on, so what is being shown is that the flat theme is what wins.
        let flat =
            with_color(|| render_with("[[RED]]red[[/]] and **bold**", options(ThemeChoice::Text)));

        assert_eq!(flat, "red and bold");
    }

    #[test]
    fn fenced_code_is_drawn_without_its_fence() {
        assert_eq!(drawn("```\nlet x = 1;\n```"), "let x = 1;");
        assert_eq!(drawn("~~~rust\nlet x = 1;\n~~~"), "let x = 1;");
    }

    #[test]
    fn a_fence_nobody_closed_takes_the_rest_of_the_message() {
        assert_eq!(drawn("```\nlet x = 1;"), "let x = 1;");
    }

    #[test]
    fn a_code_fence_is_not_read_for_styles() {
        assert_eq!(drawn("```\n[[red]]a\n```"), "[[red]]a");
    }
}
