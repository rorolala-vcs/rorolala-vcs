//! The `--format` output mode: a command's result drawn through a template.
//!
//! A query command publishes what it found into [`ResFormat`], under the key `--json` names it by,
//! and the run is drawn through the template it ends with. That template is the user's `--format`,
//! or the command's own default — which is why a query command is drawn through a template whether
//! the user asked for one or not: a command always has a way to be read.
//!
//! [`ResFormat::default_template`] is what a command calls as it is reached to name its default,
//! and [`ResFormat::set`] what it calls once it has the result. `--json` comes first: a run that
//! asked for a structural renderer leaves the template unset, so what the framework serializes is
//! what is printed and the default is never written.

use mingling::{
    ProgramCollect,
    config::StructuralRendererSetting,
    macros::arg,
    picker::{PickerArg, PickerHelper},
    setup::ProgramSetup,
};
use rorolala_utils_cli_theme::warn_line;
use rust_i18n::t;

/// Global argument that names the template a result is drawn with.
///
/// Usage: `--format="{{ ... }}"`
pub const GLOBAL_ARG_FORMAT: PickerArg<'static, String> = arg![format: String];

/// What a result is drawn with, and the data it is drawn from.
///
/// A resource, so a command reads it by injection and main reads it once the run is over: the
/// template names the shape, and the data is what the command published under one key.
#[derive(Debug, Default, Clone)]
pub struct ResFormat {
    /// The template, when the run is to be drawn through one.
    template: Option<String>,
    /// Whether the run asked for a structural renderer of its own.
    ///
    /// `--json` and its kind come first: a command leaves its default template unwritten when this
    /// is set, so what the framework serializes is what is printed.
    structured: bool,
    /// The data the template is drawn from: the key it is reached by, and one value per line.
    data: Option<(String, Vec<serde_json::Value>)>,
}

impl ResFormat {
    /// A run's format, with the template the user named, if any.
    #[must_use]
    pub(crate) fn new(template: Option<String>, structured: bool) -> Self {
        Self {
            template,
            structured,
            data: None,
        }
    }

    /// Whether a template is in force, and so whether a result is drawn through one.
    #[must_use]
    pub fn requested(&self) -> bool {
        self.template.is_some()
    }

    /// Whether a command has published the data a template is drawn from.
    ///
    /// A command that failed publishes nothing, so its failure is printed by the framework rather
    /// than drawn through a template: the template is for a result, and a run that has none has
    /// only its output.
    #[must_use]
    pub fn published(&self) -> bool {
        self.data.is_some()
    }

    /// The result drawn through the template, when the run is to be drawn through one and a
    /// command has published the data for it.
    ///
    /// A run with no template, or a command that published nothing, is `None`: the renderer draws
    /// its own form instead, so a failure is still read as it always was.
    #[must_use]
    pub fn drawn(&self) -> Option<Result<String, minijinja::Error>> {
        (self.requested() && self.published()).then(|| self.render())
    }

    /// Names the template a command is drawn with, when the run has not already decided one.
    ///
    /// A run that asked for a structural renderer keeps it, so its default is never written and
    /// `--json` stays what answers; a template the user named stands; what is left is filled with
    /// the command's own. A command calls this as it is reached, before its work, so the default
    /// holds even when the work then fails and the failure is what is drawn.
    pub fn default_template(&mut self, template: impl AsRef<str>) {
        if self.structured || self.template.is_some() {
            return;
        }

        self.template = Some(template.as_ref().to_owned());
    }

    /// Publishes what a command found, under `key`, one `values` entry to a line.
    pub fn set(&mut self, key: impl Into<String>, values: Vec<serde_json::Value>) {
        self.data = Some((key.into(), values));
    }

    /// Draws the published data through the template.
    ///
    /// Each value is drawn on its own, with the key naming it in the context: a template says
    /// `{{ key }}` for a list whose values are scalars, and `{{ key.field }}` for one whose values
    /// are objects. What a value is drawn as, and what fields it has, are the same as what `--json`
    /// writes for it, so a template and a JSON reader name the same things the same way.
    ///
    /// # Errors
    ///
    /// Returns why a template would not be read or drawn, or why a name in it is not one the data
    /// has.
    pub fn render(&self) -> Result<String, minijinja::Error> {
        let (Some(template), Some((key, values))) = (&self.template, &self.data) else {
            return Ok(String::new());
        };

        let mut environment = minijinja::Environment::new();
        environment.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
        let template = environment.template_from_str(template)?;

        let mut drawn = String::new();
        for value in values {
            drawn.push_str(&template.render(serde_json::json!({ key.as_str(): value }))?);
            drawn.push('\n');
        }

        Ok(drawn)
    }
}

/// A [`ProgramSetup`] that turns `--format` into [`ResFormat`].
pub struct FormatSetup;

impl<ThisProgram> ProgramSetup<ThisProgram> for FormatSetup
where
    ThisProgram: ProgramCollect<Enum = ThisProgram>,
{
    fn setup(self, program: &mut mingling::Program<ThisProgram>) {
        let asked = program.pick_argument(&GLOBAL_ARG_FORMAT);

        // A structural renderer is the first choice, so a run that asked for one keeps it and
        // `--format` is not applied to it. Naming both is a conflict worth saying out loud.
        let structured = !matches!(
            program.structural_renderer_name,
            StructuralRendererSetting::Disable
        );
        if structured && asked.is_some() {
            eprintln!("{}", warn_line!(t!("format.conflict_json").trim()));
        }

        let template = if structured { None } else { asked };
        program.with_resource(ResFormat::new(template, structured));
    }
}
