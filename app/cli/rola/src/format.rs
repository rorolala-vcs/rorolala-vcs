//! The `--format` output mode, re-exported from the shared CLI setups.
//!
//! It lives in `rorolala-cli-setups` because it is not this program's alone: `rola-daemon`
//! answers with the same templates and the same `--json` first rule. What is here is the name
//! this program's command modules already reach for.

pub use rorolala_cli_setups::{FormatSetup, ResFormat};
