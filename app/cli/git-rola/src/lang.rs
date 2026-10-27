//! The language a run speaks, chosen the way the `rola` program chooses it.
//!
//! `--lang`/`-l` names one, and the environment is asked when it does not, in the order
//! `ROLA_LANG`, `APP_LANG` and `LANG`. The choice is made during setup, before any command runs, so
//! every `t!` after it resolves in the selected locale.
//!
//! The code is here rather than reached for from `rorolala-cli-setups` because that crate links
//! `librorolala`: this program is the git half alone, and it must not carry a Workspace, a Layout,
//! the Vault or the store in with it just to know which language to speak.

use std::env;

use mingling::{
    ProgramCollect,
    macros::arg,
    picker::{PickerArg, PickerHelper},
    setup::ProgramSetup,
};

/// The environment variables consulted, in order, when `--lang` is absent.
///
/// The first one set wins. `ROLA_LANG` is the project's own; `APP_LANG` and `LANG` are the
/// conventions a container and a shell already carry.
const LANGUAGE_ENV_VARS: &[&str] = &["ROLA_LANG", "APP_LANG", "LANG"];

/// The language used when neither `--lang` nor the environment names one.
const FALLBACK_LANGUAGE: &str = "en";

/// The global argument selecting the language: `--lang zh-CN` or `-l zh-CN`.
const GLOBAL_ARG_LANG: PickerArg<'static, String> = arg![lang: String, 'l'];

/// Selects the language and binds it to `rust-i18n` before any command runs.
pub struct LanguageSetup;

impl<C> ProgramSetup<C> for LanguageSetup
where
    C: ProgramCollect<Enum = C>,
{
    fn setup(self, program: &mut mingling::Program<C>) {
        let language = program
            .pick_argument(&GLOBAL_ARG_LANG)
            .filter(|language| !language.is_empty())
            .map_or_else(default_language, |language| locale_of(&language));

        rust_i18n::set_locale(&language);
    }
}

/// The language to run in when `--lang` named none: the environment, or the fallback.
fn default_language() -> String {
    LANGUAGE_ENV_VARS
        .iter()
        .find_map(|name| env::var(name).ok())
        .filter(|language| !language.is_empty())
        .map_or_else(
            || FALLBACK_LANGUAGE.to_string(),
            |language| locale_of(&language),
        )
}

/// An environment value as a locale `rust-i18n` knows.
///
/// `LANG` and friends carry an encoding suffix and spell the territory with an underscore —
/// `zh_CN.UTF-8` — and neither belongs in a locale name, so they become `zh-CN`. A value already in
/// that shape passes through unchanged.
fn locale_of(value: &str) -> String {
    value.split('.').next().unwrap_or(value).replace('_', "-")
}
