# rorolala-utils-cli-theme

The styles Rorolala's command-line programs are drawn in.

A library of style functions rather than a theme object: a caller asks for the piece it
needs, so adding a style is adding a function, and nothing that is already on screen
depends on it.

## Coloring

The switch that decides whether any of it is colored at all lives here, because every
style in this crate reads it. `Colorize` is re-exported rather than wrapped, so a call
site is written the way it would be with `colored` itself — there is no second
vocabulary to learn:

```rust
use rorolala_utils_cli_theme::{Colorize, is_enabled, set_enabled};

set_enabled(false);
assert!(!is_enabled());
assert_eq!(format!("{}", "plain".red()), "plain");

set_enabled(true);
assert!(is_enabled());
```

Until a program states its own answer, the environment decides — `CLICOLOR_FORCE`,
`NO_COLOR`, then `CLICOLOR` together with whether standard output is a terminal — which
is the rule `colored` already follows. Stating one takes that decision over, for
everything in the workspace that colors through `colored`.

## Themes

How much a style may ask of the terminal is `ThemeChoice`, and it is global for the same
reason the coloring switch is — every style reads it:

```rust
use rorolala_utils_cli_theme::{ThemeChoice, set_theme_choice, theme_choice};

set_theme_choice(ThemeChoice::Pretty);
assert_eq!(theme_choice(), ThemeChoice::Pretty);
```

`ThemeChoice::auto_select` is what the choice is until a program states one: the
terminal's own answer, which is as much as anything can know. It never returns `Pretty`,
because nothing can ask a terminal whether its font carries the glyphs or its color is
24-bit deep — asking for `Pretty` stays a decision a person makes.

## Styles

A message is written the way it should read and drawn once, by the `trd!` macro — or,
where a macro does not fit, by `TextRendering::render`. What the drawing may use is
read off the theme and the coloring switch, so one message draws itself as plain text
on a terminal that wants none of it:

```rust
use rorolala_utils_cli_theme::{set_enabled, trd};

set_enabled(false);
assert_eq!(trd!("Hello, **world**!"), "Hello, world!");
```

### Blocks

The blocks are the ones a person already knows from Markdown:

| Written | Drawn |
| --- | --- |
| `# Title` | a title, in bold up to the third level, with a blank line either side |
| `> quoted` | `\| quoted`, grey |
| `> [!Warning]` | an alert, headed by its name |
| `----` | a rule, see below |
| `\| a \| b \|` | a table |
| a ` ```rust ` fence | code, highlighted by [`bat`](https://github.com/sharkdp/bat) |

A rule is drawn in the alphabet of the theme that draws it: plain `-` across the width
of the message on most themes, and the box-drawing `─` across the whole terminal on a
theme that carries the glyphs. The terminal's width is asked for rather than assumed,
so a message going into a file falls back to the width of the message either way.

A table is drawn in the same two alphabets, and changes between them at the same
place: `+` and `-` where the glyphs are missing, and `┌ ─ ┬ ┐ │ ├ ┼ ┤ └ ┴ ┘` where they
are there. Styles inside a cell are drawn like any others.

An alert is headed by its name in upper case on a theme without the glyphs:

```text
| WARNING:
|
| something went wrong
```

and by its mark on one with them:

```text
|  Warning
|
| something went wrong
```

### Inline

Styles inside a line are marked with a smaller, deliberately non-Markdown set, because
a message is not a document: a `[[red]]` stays on until its `[[/]]`, and directives
nest, where Markdown's emphasis only ever pairs up.

| Written | Drawn |
| --- | --- |
| `[[red]]` … `[[/]]` | red, `[[RED]]` being the bright one |
| `[[~red]]` … `[[/]]` | red behind the text |
| `[[#FF0000]]` … `[[/]]` | an exact colour |
| `[[?https://example.com]]` … `[[/]]` | a link a terminal can follow |
| `**bold**` | bold |
| `*italic*` | italic |
| `***both***` | bold and italic |
| `__underlined__` | underlined |
| `~~struck~~` | struck through |
| `` `code` `` | bold and bright cyan, markers kept |
| `<flag>` | italic and bright cyan, markers kept |

A backslash takes the meaning away from `*`, `_` and `~`, and names a character by
number as `\u2764` or `\u{2764}`.

A delimiter opens only where the same one closes later on the line, so that `2 * 3`
stays arithmetic.
