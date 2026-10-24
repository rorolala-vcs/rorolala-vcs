//! Build script: generate the registry of chunking policies.
//!
//! Every policy is an `impl Policy` under `src/policies`, and the table one is found through is
//! generated from those impls rather than written by hand, so adding a policy is adding one file.
//! It is the shape `modules/daemon/build.rs` builds the action registry with, and for the same
//! reason: a list everybody edits is a list everybody conflicts on.
//!
//! The impls are read with `syn`, which sees exactly what a reader sees. What is taken from one is
//! its `NAME` and its `EXTENSIONS`; the table then names those consts rather than copying the
//! literals, so a policy's own consts stay the only place they are written down.
//!
//! Three things stop the build rather than becoming a surprise at run time: two policies with one
//! name, two policies claiming one container, and a name or a container this crate would not accept.
//! The first two are what let several people add several formats without ever colliding, and they
//! are checked here because a build is the earliest place they can be.
//!
//! What comes out is shaped by `tmpl/`, so the wording of a generated file can change without
//! touching this script. Both are written into the source tree rather than into `OUT_DIR`, so a
//! change to the registry is visible in the diff beside the policy that caused it.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

use just_template::Template;
use syn::{Expr, ExprArray, ExprLit, ImplItem, Item, Lit, Type};

/// Where the policies are read from, relative to the manifest directory.
const POLICIES_DIR: &str = "src/policies";

/// The template the module list is rendered from.
const MODULES_TEMPLATE: &str = "tmpl/mod.tmpl";

/// The template the registry is rendered from.
const REGISTRY_TEMPLATE: &str = "tmpl/registry.tmpl";

/// The file the module list is written to.
const MODULES_OUTPUT: &str = "src/policies/mod.rs";

/// The file the registry is written to.
const REGISTRY_OUTPUT: &str = "src/registry.rs";

/// The trait a policy implements, matched by the last segment of its path.
const POLICY_TRAIT: &str = "Policy";

/// The associated constant a policy declares its name in.
const POLICY_NAME: &str = "NAME";

/// The associated constant a policy declares the containers it claims in.
const POLICY_EXTENSIONS: &str = "EXTENSIONS";

/// The template parameter the module declarations go.
const MODULES_PARAM: &str = "modules";

/// The template parameter the table's rows go.
const ROWS_PARAM: &str = "rows";

fn main() {
    println!("cargo:rerun-if-changed={POLICIES_DIR}");
    println!("cargo:rerun-if-changed={MODULES_TEMPLATE}");
    println!("cargo:rerun-if-changed={REGISTRY_TEMPLATE}");
    println!("cargo:rerun-if-changed=build.rs");

    // UNWRAP: cargo sets `CARGO_MANIFEST_DIR` for every build script it runs, so this is a
    // build-script contract rather than a runtime input.
    let manifest = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR for build scripts"),
    );
    let directory = manifest.join(POLICIES_DIR);

    // Made rather than expected. A fresh clone may hold no policy at all — the crate builds with an
    // empty table — and git does not carry a directory with nothing in it, so refusing a missing one
    // would make the first policy the only way to compile.
    if let Err(error) = fs::create_dir_all(&directory) {
        fail(&format!("making {}: {error}", directory.display()));
    }

    let mut files = match fs::read_dir(&directory) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
            // The module list written here is itself one of the files; it declares no policy.
            .filter(|path| path != &manifest.join(MODULES_OUTPUT))
            // A draft parks itself under `__`, the way `.gitignore` says.
            .filter(|path| {
                !path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("__"))
            })
            .collect::<Vec<_>>(),
        Err(error) => fail(&format!("reading {}: {error}", directory.display())),
    };
    files.sort();

    let mut policies = Vec::new();
    let mut modules = Vec::new();
    for path in files {
        // Every file under the policies directory is a module of it, whether or not it declares a
        // policy: what the policy side needs that is not a policy lives beside them — a parser, a
        // table of a format's block kinds — and declaring it is what lets it be reached. Only a
        // policy gets a row, so a file that declares none contributes nothing to the table.
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            fail(&format!("{}: the file name is not text", path.display()));
        };
        modules.push(stem.to_owned());

        policies.extend(policies_in(&path).unwrap_or_else(|error| fail(&error)));
    }

    if let Err(error) = check(&policies) {
        fail(&error);
    }
    policies.sort_by(|left, right| left.name.cmp(&right.name));

    write_if_changed(
        &manifest.join(MODULES_OUTPUT),
        &render_modules(&manifest, &modules),
    );
    write_if_changed(
        &manifest.join(REGISTRY_OUTPUT),
        &render_registry(&manifest, &policies),
    );
}

/// One policy, as its `impl Policy for ...` block spells it.
struct Declared {
    /// The registered name: what a store is told to use it by.
    name: String,
    /// The containers it claims.
    extensions: Vec<String>,
    /// The implementing type, as written.
    type_name: String,
    /// The module the type is in, which is the file it was declared in.
    module: String,
}

/// Reads one file and returns the policies its `impl Policy` blocks declare.
fn policies_in(path: &Path) -> Result<Vec<Declared>, String> {
    let source =
        fs::read_to_string(path).map_err(|error| format!("reading {}: {error}", path.display()))?;
    let file =
        syn::parse_file(&source).map_err(|error| format!("parsing {}: {error}", path.display()))?;

    let module = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| format!("{}: the file name is not text", path.display()))?
        .to_owned();

    file.items
        .iter()
        .filter_map(|item| policy_of(item, &module).transpose())
        .map(|policy| policy.map_err(|error| format!("{}: {error}", path.display())))
        .collect()
}

/// The policy one item declares, if the item is an `impl Policy for ...`.
///
/// An item that is not an impl of `Policy` — an impl of another trait, a function, a type — yields
/// `None`, so a file can hold whatever else it likes. An `impl Policy` that cannot be read is an
/// error instead: leaving it out would leave a policy out of the table without saying so.
fn policy_of(item: &Item, module: &str) -> Result<Option<Declared>, String> {
    let Item::Impl(item) = item else {
        return Ok(None);
    };

    let Some((trait_path, _)) = &item.trait_ else {
        return Ok(None);
    };
    if trait_path
        .segments
        .last()
        .is_none_or(|segment| segment.ident != POLICY_TRAIT)
    {
        return Ok(None);
    }

    let type_name = plain_name(&item.self_ty)
        .ok_or_else(|| "a policy is implemented for a plain type name".to_owned())?;

    let mut name = None;
    let mut extensions = None;
    for item in &item.items {
        match item {
            ImplItem::Const(associated) if associated.ident == POLICY_NAME => {
                name = Some(text(&associated.expr).ok_or_else(|| {
                    format!("`{type_name}`: `{POLICY_NAME}` must be a string literal")
                })?);
            }
            ImplItem::Const(associated) if associated.ident == POLICY_EXTENSIONS => {
                extensions = Some(list(&associated.expr).ok_or_else(|| {
                    format!(
                        "`{type_name}`: `{POLICY_EXTENSIONS}` must be an array of string literals"
                    )
                })?);
            }
            _ => {}
        }
    }

    let name = name.ok_or_else(|| format!("`{type_name}` declares no `{POLICY_NAME}`"))?;
    let extensions =
        extensions.ok_or_else(|| format!("`{type_name}` declares no `{POLICY_EXTENSIONS}`"))?;

    Ok(Some(Declared {
        name,
        extensions,
        type_name,
        module: module.to_owned(),
    }))
}

/// The text of a string literal, if that is what `expression` is.
fn text(expression: &Expr) -> Option<String> {
    let Expr::Lit(ExprLit {
        lit: Lit::Str(text),
        ..
    }) = expression
    else {
        return None;
    };

    Some(text.value())
}

/// The texts of a slice of string literals, if that is what `expression` is.
fn list(expression: &Expr) -> Option<Vec<String>> {
    let expression = match expression {
        Expr::Reference(reference) => reference.expr.as_ref(),
        other => other,
    };

    let Expr::Array(ExprArray { elems, .. }) = expression else {
        return None;
    };

    elems.iter().map(text).collect()
}

/// The name of a plain path type: no qualification, no generics.
fn plain_name(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else {
        return None;
    };

    if path.qself.is_some() || path.path.segments.len() != 1 {
        return None;
    }

    let segment = path.path.segments.last()?;
    if !matches!(segment.arguments, syn::PathArguments::None) {
        return None;
    }

    Some(segment.ident.to_string())
}

/// Refuses the policies that would make the table mean two things at once.
fn check(policies: &[Declared]) -> Result<(), String> {
    let mut names: HashMap<&str, &str> = HashMap::new();
    let mut claims: HashMap<&str, &str> = HashMap::new();

    for policy in policies {
        if !kebab(&policy.name) {
            return Err(format!(
                "`{}`: `{}` is not kebab-case, and a name is what a store is told to use it by",
                policy.type_name, policy.name
            ));
        }

        if let Some(other) = names.insert(&policy.name, &policy.type_name) {
            return Err(format!(
                "`{}` and `{}` both claim the name `{}`",
                other, policy.type_name, policy.name
            ));
        }

        for extension in &policy.extensions {
            if !container(extension) {
                return Err(format!(
                    "`{}`: `{extension}` is not a container, which is a lowercase extension with no dot",
                    policy.type_name
                ));
            }

            if let Some(other) = claims.insert(extension, &policy.type_name) {
                return Err(format!(
                    "`{}` and `{}` both claim `{extension}`, and one container is cut one way",
                    other, policy.type_name
                ));
            }
        }
    }

    Ok(())
}

/// Whether `name` is kebab-case: lowercase words joined by one hyphen each.
fn kebab(name: &str) -> bool {
    !name.is_empty()
        && name.split('-').all(|word| {
            !word.is_empty()
                && word
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
        })
}

/// Whether `extension` is a container as this crate spells one: lowercase, no dot.
fn container(extension: &str) -> bool {
    !extension.is_empty()
        && extension
            .chars()
            .all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())
}

/// Renders the module list from its template.
fn render_modules(manifest: &Path, modules: &[String]) -> String {
    let mut template = Template::from(read(manifest, MODULES_TEMPLATE));
    template.insert_param(MODULES_PARAM.to_owned(), declarations(modules));

    expand(template, MODULES_TEMPLATE)
}

/// What each policy file needs said about it: its module, and what that module exports.
fn declarations(modules: &[String]) -> String {
    modules
        .iter()
        .map(|module| format!("pub mod {module};\npub use {module}::*;"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Renders the registry from its template.
fn render_registry(manifest: &Path, policies: &[Declared]) -> String {
    let mut template = Template::from(read(manifest, REGISTRY_TEMPLATE));

    let rows = policies
        .iter()
        .map(|policy| {
            format!(
                "    crate::PolicyEntry {{\n        \
                 name: crate::policies::{module}::{type_name}::{POLICY_NAME},\n        \
                 extensions: crate::policies::{module}::{type_name}::{POLICY_EXTENSIONS},\n        \
                 chunker: &crate::policies::{module}::{type_name},\n    }},",
                module = policy.module,
                type_name = policy.type_name,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    template.insert_param(ROWS_PARAM.to_owned(), rows);

    expand(template, REGISTRY_TEMPLATE)
}

/// Reads a template, or stops the build saying which one could not be read.
fn read(manifest: &Path, template: &str) -> String {
    match fs::read_to_string(manifest.join(template)) {
        Ok(source) => source,
        Err(error) => fail(&format!("reading {template}: {error}")),
    }
}

/// Expands a rendered template, or stops the build rather than writing half of one.
fn expand(template: Template, name: &str) -> String {
    template.expand().map_or_else(
        || {
            fail(&format!(
                "expanding {name}: its blocks do not pair up, or a block is nested"
            ))
        },
        // `expand` trims what it produces, so the newline a text file ends with is put back here.
        |rendered| rendered + "\n",
    )
}

/// Writes `contents` to `path`, unless it already says exactly that.
///
/// Rewriting a file that has not changed would give it a new timestamp, and a build that touched the
/// sources on every run would make every later build think something had moved.
fn write_if_changed(path: &Path, contents: &str) {
    if fs::read_to_string(path).is_ok_and(|existing| existing == contents) {
        return;
    }

    if let Err(error) = fs::write(path, contents) {
        fail(&format!("writing {}: {error}", path.display()));
    }
}

/// Reports `message` to cargo and stops the build.
///
/// A build script can only speak to cargo through its directives, so the diagnostic is forwarded
/// line by line and the script then exits non-zero: a table that is only half generated must never
/// be left behind as if it were whole.
fn fail(message: &str) -> ! {
    for line in message.lines() {
        println!("cargo:warning={line}");
    }

    exit(1);
}
