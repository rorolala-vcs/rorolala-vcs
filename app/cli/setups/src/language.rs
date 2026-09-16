use std::env;

use mingling::{
    ProgramCollect, Wrap,
    macros::arg,
    picker::{PickerArg, PickerHelper},
    setup::ProgramSetup,
};

/// Environment variables consulted, in order, when `--lang` is absent
///
/// The first one set wins. `ROROLALA_LANG` is the project's own; `APP_LANG` and `LANG`
/// are the conventions a shell and a container already carry.
const LANGUAGE_ENV_VARS: &[&str] = &["ROROLALA_LANG", "APP_LANG", "LANG"];

/// Language used when neither `--lang` nor the environment names one
const FALLBACK_LANGUAGE: &str = "en";

/// Global argument selecting the language
///
/// Usage: `--lang zh-CN` or `-l zh-CN`
pub const GLOBAL_ARG_LANG: PickerArg<'static, String> = arg![lang: String, 'l'];

/// A [`ProgramSetup`] implementation that selects the language and binds it to rust-i18n
///
/// The language comes from `--lang`, or — when the argument is absent — from the first
/// of `ROROLALA_LANG`, `APP_LANG` and `LANG` that is set, falling back to `en`.
///
/// The choice is made during setup, so no command has run yet when it lands and every
/// `t!` after it resolves in the selected locale. That works across crates: the locale
/// is state of the `rust-i18n` instance the whole program links, while the translations
/// themselves are compiled into whichever crate called `i18n!`.
pub struct LanguageSetup;

/// Resource representing the language the current program runs in
#[derive(Debug, Default, Clone, Wrap)]
pub struct ResLanguage(String);

impl<ThisProgram> ProgramSetup<ThisProgram> for LanguageSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        let language = program
            .pick_argument(&GLOBAL_ARG_LANG)
            .filter(|language| !language.is_empty())
            .map_or_else(default_language, |language| locale_of(&language));

        rust_i18n::set_locale(&language);
        program.with_resource(ResLanguage::from(language));
    }
}

/// The language to run in when `--lang` did not name one: the environment, or the
/// fallback.
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

/// An environment value as a locale rust-i18n knows
///
/// `LANG` and friends carry an encoding suffix and spell the territory with an
/// underscore — `zh_CN.UTF-8` — and neither belongs in a locale name, so they become
/// `zh-CN`. A value already in that shape passes through unchanged.
fn locale_of(value: &str) -> String {
    value.split('.').next().unwrap_or(value).replace('_', "-")
}
