//! The `--format` output mode: a command's result drawn through a template.
//!
//! A query command publishes what it found into [`ResFormat`], under the key `--json` names it by,
//! and the run is drawn through the template it ends with. That template is the user's `--format`,
//! or the command's own default — which is why a query command is drawn through a template whether
//! the user asked for one or not: a command always has a way to be read.
//!
//! How it is drawn turns on how many lists the command published. One list is the common case —
//! `entries`, `accounts`, one array `--json` writes — and is drawn a row at a time, the template to
//! a value, so `{{ entries.path }}` is a line per entry. A reading whose parts are lists of their
//! own — `layout tree-diff` has five — has no single row, so every key is bound at once and the
//! template is drawn once: a reader writes `{{ lost | join('\n') }}` for the lines of one of them.
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
    /// The data the template is drawn from: each list a command published, by the key `--json`
    /// names it by, and one value to a row.
    data: Vec<(String, Vec<serde_json::Value>)>,
}

impl ResFormat {
    /// A run's format, with the template the user named, if any.
    #[must_use]
    pub(crate) fn new(template: Option<String>, structured: bool) -> Self {
        Self {
            template,
            structured,
            data: Vec::new(),
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
        !self.data.is_empty()
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

    /// Publishes what a command found, under `key`.
    ///
    /// Publishing a key again adds to what it already holds, so a command that finds more of one
    /// list as it goes says so by publishing more; a key published once is the whole of it.
    pub fn set(&mut self, key: impl Into<String>, values: Vec<serde_json::Value>) {
        let key = key.into();

        if let Some((_, held)) = self.data.iter_mut().find(|(name, _)| *name == key) {
            held.extend(values);
        } else {
            self.data.push((key, values));
        }
    }

    /// Draws the published data through the template.
    ///
    /// One list is drawn a row at a time, with the key naming it in the context: a template says
    /// `{{ key }}` for a list whose values are scalars, and `{{ key.field }}` for one whose values
    /// are objects. Several lists are bound at once and the template is drawn once, every key an
    /// array: a reader names a whole list, as `{{ key | length }}` or `{{ key | join('\n') }}`.
    /// Either way, what a value is drawn as, and what fields it has, are the same as what `--json`
    /// writes for it, so a template and a JSON reader name the same things the same way. A drawing
    /// that does not end in a newline is given one, so what is read is a line of its own.
    ///
    /// # Errors
    ///
    /// Returns why a template would not be read or drawn, or why a name in it is not one the data
    /// has.
    pub fn render(&self) -> Result<String, minijinja::Error> {
        let Some(template) = &self.template else {
            return Ok(String::new());
        };

        let mut environment = minijinja::Environment::new();
        environment.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
        let template = environment.template_from_str(template)?;

        match self.data.as_slice() {
            [] => Ok(String::new()),
            [(key, values)] => {
                let mut drawn = String::new();
                for value in values {
                    drawn.push_str(&template.render(serde_json::json!({ key.as_str(): value }))?);
                    drawn.push('\n');
                }
                Ok(drawn)
            }
            lists => {
                let bound = serde_json::Value::Object(
                    lists
                        .iter()
                        .map(|(key, values)| (key.clone(), serde_json::json!(values)))
                        .collect(),
                );

                let mut drawn = template.render(bound)?;
                if !drawn.is_empty() && !drawn.ends_with('\n') {
                    drawn.push('\n');
                }
                Ok(drawn)
            }
        }
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

#[cfg(test)]
mod tests {
    use super::ResFormat;
    use serde_json::json;

    /// A format drawn through `template` as a run that named it would be.
    fn format(template: &str) -> ResFormat {
        ResFormat::new(Some(template.to_owned()), false)
    }

    #[test]
    fn one_list_is_drawn_a_row_at_a_time() {
        let mut format = format("{{ entries.path }}");
        format.set(
            "entries",
            vec![json!({ "path": "a" }), json!({ "path": "b" })],
        );

        assert_eq!(format.render().expect("drawn"), "a\nb\n");
    }

    #[test]
    fn several_lists_are_bound_at_once() {
        let mut format = format("{{ lost | join('\\n') }}|{{ untagged | length }}");
        format.set("lost", vec![json!("a"), json!("b")]);
        format.set("untagged", vec![json!("c")]);

        assert_eq!(format.render().expect("drawn"), "a\nb|1\n");
    }

    #[test]
    fn publishing_a_key_again_adds_to_it() {
        let mut format = format("{{ x }}");
        format.set("x", vec![json!("a")]);
        format.set("x", vec![json!("b")]);

        assert_eq!(format.render().expect("drawn"), "a\nb\n");
    }

    #[test]
    fn a_name_the_data_does_not_have_is_refused() {
        let mut format = format("{{ nope }}");
        format.set("x", vec![json!("a")]);

        assert!(format.render().is_err());
    }
}
