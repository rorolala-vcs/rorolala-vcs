use std::env;

use mingling::{
    ProgramCollect, Wrap,
    consts::REMAINS,
    macros::arg,
    picker::{IntoPicker, Pickable},
    setup::ProgramSetup,
};

/// Environment variables consulted, in order, when `--lang` is absent.
///
/// The first one set wins. `ROROLALA_LANG` is the project's own; `APP_LANG` and `LANG`
/// are the conventions a shell and a container already carry.
pub const DEFAULT_LANGUAGE_ENV_VARS: &[&str] = &["ROROLALA_LANG", "APP_LANG", "LANG"];

/// Language used when neither `--lang` nor the environment names one.
pub const DEFAULT_LANGUAGE: &str = "en";

/// The language this run is in.
///
/// A resource rather than only the global locale, because the language is an input to
/// more than `t!` lookups: help documents and completion text are addressed by name,
/// and are not compiled in.
#[derive(Debug, Default, Clone, Wrap)]
pub struct ResLanguage {
    /// The locale the program selected.
    language: String,
}

/// The global arguments that select the language.
#[derive(Pickable)]
pub struct LanguageFlags {
    /// The locale to run in, such as `en` or `zh-CN`.
    ///
    /// When omitted, the environment decides — see
    /// [`DEFAULT_LANGUAGE_ENV_VARS`] — and [`DEFAULT_LANGUAGE`] is the last resort.
    #[arg(short, long)]
    lang: Option<String>,
}

/// A [`ProgramSetup`] that picks the language and binds it to rust-i18n.
///
/// The choice is made during setup, so no command has run yet when it lands and every
/// `t!` after it resolves in the selected locale. That works across crates: the
/// locale is state of the `rust-i18n` instance the whole program links, while the
/// translations themselves are compiled into whichever crate called `i18n!`.
pub struct LanguageSetup {
    /// Environment variables consulted, in order, when `--lang` is absent.
    env_vars: Vec<String>,
    /// Language used when neither the argument nor the environment names one.
    fallback: String,
}

impl Default for LanguageSetup {
    fn default() -> Self {
        Self {
            env_vars: DEFAULT_LANGUAGE_ENV_VARS
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
            fallback: DEFAULT_LANGUAGE.to_string(),
        }
    }
}

impl LanguageSetup {
    /// A setup that reads `--lang`, then [`DEFAULT_LANGUAGE_ENV_VARS`], then
    /// [`DEFAULT_LANGUAGE`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an environment variable for this program to consult.
    ///
    /// Consulted before the defaults, so a program can name its own without giving up
    /// the conventions underneath.
    #[must_use]
    pub fn with_env_var(mut self, name: impl Into<String>) -> Self {
        self.env_vars.insert(0, name.into());
        self
    }

    /// Sets the language used when neither the argument nor the environment names one.
    #[must_use]
    pub fn with_fallback(mut self, language: impl Into<String>) -> Self {
        self.fallback = language.into();
        self
    }
}

impl<ThisProgram> ProgramSetup<ThisProgram> for LanguageSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        // UNWRAP: `pick` for both `LanguageFlags` and `REMAINS` is infallible here — each
        // pick can fall back, so parsing never fails and the `unwrap` is safe.
        let (flags, args) = program
            .take_args()
            .pick(&arg![LanguageFlags])
            .pick(&REMAINS)
            .unwrap();
        program.replace_args(args.into());

        let language = resolve_language(flags.lang, &self.env_vars, &self.fallback);
        rust_i18n::set_locale(&language);
        program.with_resource(ResLanguage { language });
    }
}

/// The language to run in: the argument if it named one, then the environment, then
/// the fallback.
fn resolve_language(requested: Option<String>, env_vars: &[String], fallback: &str) -> String {
    requested
        .filter(|language| !language.is_empty())
        .or_else(|| {
            env_vars
                .iter()
                .find_map(|name| env::var(name).ok())
                .filter(|language| !language.is_empty())
        })
        .map_or_else(|| fallback.to_string(), |language| locale_of(&language))
}

/// An environment value as a locale rust-i18n knows.
///
/// `LANG` and friends carry an encoding suffix and spell the territory with an
/// underscore — `zh_CN.UTF-8` — and neither belongs in a locale name, so they become
/// `zh-CN`. A value already in that shape passes through unchanged.
fn locale_of(value: &str) -> String {
    value.split('.').next().unwrap_or(value).replace('_', "-")
}
