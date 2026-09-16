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

## Styles

No style functions yet.
