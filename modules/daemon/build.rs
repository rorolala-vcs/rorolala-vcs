//! Build script: generate the entry points for the daemon's actions.
//!
//! Every action is an `impl Action for ...` under `src/actions`, and each one needs the
//! same two entry points: an `async` one over `proc_action`, and a blocking one to
//! export, because `#[lazyffi]` cannot export an `async fn`. They are generated from the
//! impls themselves rather than written out per action, so adding an action is adding
//! one file.
//!
//! The impls are read with `syn`, which sees exactly what a reader sees: `type Input`
//! and `type Output` are taken as written, and every other shape — generics, an
//! `impl` that is not of `Action` — is left alone rather than guessed at.
//!
//! An input that is a tuple is spread over one parameter per element, because that is what
//! a caller would have written out by hand: `type Input = (String, i32)` gives
//! `name: String, value: i32`. The names come from the input's own doc comments, one list
//! item per parameter — `- name: String` — and fall back to `p0`, `p1`, … where there is
//! none. A single value takes its name from the action's own `process`, which already named
//! it — `name` in `async fn process(name: OnlyWorkspace<Self::Input>, …)` — because that is
//! the name the action uses for it everywhere else; a method that does not name it plainly
//! falls back to `input`.
//!
//! What comes out is shaped by `tmpl/action_func.tmpl`, one arm per action, so the
//! wording of an entry point can be changed without touching this script.
//!
//! A parameter is spelled `<<<name>>>`. Where one stands for a generic argument, the
//! template writes its opening brackets straight onto the generic's own, as in
//! `Result<<<<output>>>`. Substitution is a plain string replacement, which finds the
//! delimiter from its second `<` on, so that run of angle brackets is deliberate: the
//! first one belongs to `Result`, the other three open the parameter.
//!
//! The file is written into the source tree, as `src/actions/action_func.rs`, and not
//! into `OUT_DIR`. The C header is generated from the sources — it expands no macros —
//! so an export the header generator cannot read is an export the header does not have.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

use just_template::Template;
use syn::{
    Attribute, Expr, ExprLit, FnArg, GenericArgument, ImplItem, ImplItemFn, Item, Lit, Meta, Pat,
    PathArguments, Type, TypePath,
};

/// Where the actions are read from, relative to the manifest directory.
const ACTIONS_DIR: &str = "src/actions";

/// The template the entry points are rendered from, relative to the manifest directory.
const TEMPLATE: &str = "tmpl/action_func.tmpl";

/// The template the registry is rendered from, relative to the manifest directory.
const REGISTRY_TEMPLATE: &str = "tmpl/registry.tmpl";

/// The template's implementation block, which is repeated once per action.
const ACTIONS_BLOCK: &str = "actions";

/// The file the entry points are written to, relative to the manifest directory.
const OUTPUT: &str = "src/action_func.rs";

/// The file the registry is written to, relative to the manifest directory.
const REGISTRY_OUTPUT: &str = "src/registry.rs";

/// The template the action files' module is rendered from, relative to the manifest
/// directory.
const MODULES_TEMPLATE: &str = "tmpl/mod.tmpl";

/// The file the module declaring the action files is written to, relative to the manifest
/// directory.
const MODULES_OUTPUT: &str = "src/actions/mod.rs";

/// The trait an action implements, matched by the last segment of its path.
const ACTION_TRAIT: &str = "Action";

/// The associated constant an action declares its id in.
const ACTION_ID: &str = "ID";

/// The method an action carries its behavior out in.
const ACTION_PROCESS: &str = "process";

/// The wrapper the input arrives in, matched by the last segment of its path.
const ONLY_WORKSPACE: &str = "OnlyWorkspace";

fn main() {
    println!("cargo:rerun-if-changed={ACTIONS_DIR}");
    println!("cargo:rerun-if-changed={TEMPLATE}");
    println!("cargo:rerun-if-changed={REGISTRY_TEMPLATE}");
    println!("cargo:rerun-if-changed={MODULES_TEMPLATE}");
    println!("cargo:rerun-if-changed=build.rs");

    let manifest = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR for build scripts"),
    );
    let directory = manifest.join(ACTIONS_DIR);

    let mut action_files = match fs::read_dir(&directory) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
            // A draft parks itself under `__`, the way `.gitignore` says. A draft holds an
            // `impl Action` the module tree does not have, and an entry point generated
            // for it would be an entry point into nothing.
            .filter(|path| {
                !path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("__"))
            })
            .filter(|path| path != &manifest.join(OUTPUT))
            .filter(|path| path != &manifest.join(REGISTRY_OUTPUT))
            .filter(|path| path != &manifest.join(MODULES_OUTPUT))
            .collect::<Vec<_>>(),
        Err(error) => fail(&format!("reading {}: {error}", directory.display())),
    };
    action_files.sort();

    let mut actions = Vec::new();
    let mut modules = Vec::new();
    for path in action_files {
        let found = actions_in(&path).unwrap_or_else(|error| fail(&error));

        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            fail(&format!("{}: the file name is not text", path.display()));
        };
        // Every file under the actions directory is a module of it, whether or not it declares an
        // action: what the action side needs that is not an action lives beside them — a primitive
        // an action is built from, say — and declaring it is what lets it be reached. Only an
        // action gets entry points, so a file that declares none contributes nothing to the two
        // generated files.
        modules.push(stem.to_owned());

        actions.extend(found);
    }

    if let Err(error) = ensure_unique_ids(&actions) {
        fail(&error);
    }
    actions.sort_by_key(|action| action.id);

    for (template, output) in [(TEMPLATE, OUTPUT), (REGISTRY_TEMPLATE, REGISTRY_OUTPUT)] {
        write_if_changed(
            &manifest.join(output),
            &render(&manifest, template, &actions),
        );
    }

    write_if_changed(
        &manifest.join(MODULES_OUTPUT),
        &render_modules(&manifest, &modules),
    );
}

/// One action, as its `impl Action for ...` block spells it.
struct Action {
    /// The id it declares: the slot it takes, and the name a caller reaches it by.
    id: u32,
    /// The implementing type, as written.
    type_name: String,
    /// The parameters the entry points take for the action's input, as a signature spells
    /// them: one parameter holding a single input, or one parameter per element of a tuple.
    params: String,
    /// What the entry points hand on for that input: the parameter, or a tuple of them.
    argument: String,
    /// The same, spelled as the arguments of a call — the names alone, since a call that
    /// takes the parameters one by one is handed the names and not a tuple.
    call_args: String,
    /// What the action yields, as written.
    output: String,
}

impl Action {
    /// The stem the entry points are named after: the type's name without its `Action`
    /// prefix, in snake case, so `ActionHandshake` gives `handshake`.
    fn stem(&self) -> String {
        let bare = self
            .type_name
            .strip_prefix("Action")
            .unwrap_or(&self.type_name);

        snake_case(bare)
    }
}

/// Reads one file and returns the actions its `impl Action` blocks declare.
fn actions_in(path: &Path) -> Result<Vec<Action>, String> {
    let source =
        fs::read_to_string(path).map_err(|error| format!("reading {}: {error}", path.display()))?;
    let file =
        syn::parse_file(&source).map_err(|error| format!("parsing {}: {error}", path.display()))?;

    file.items
        .iter()
        .filter_map(|item| action_of(item).transpose())
        .map(|action| action.map_err(|error| format!("{}: {error}", path.display())))
        .collect()
}

/// The action one item declares, if the item is an `impl Action for ...`.
///
/// An item that is not an `impl` of `Action` — an impl of another trait, a function, a
/// type — yields `None`, so a file can hold whatever else it likes. An `impl Action` that
/// cannot be read is an error instead: leaving it out would leave an action out of the
/// registry without saying so.
fn action_of(item: &Item) -> Result<Option<Action>, String> {
    let Item::Impl(item) = item else {
        return Ok(None);
    };

    let Some((trait_path, _)) = &item.trait_ else {
        return Ok(None);
    };
    if trait_path
        .segments
        .last()
        .is_none_or(|segment| segment.ident != ACTION_TRAIT)
    {
        return Ok(None);
    }

    let type_name = plain_name(&item.self_ty)
        .ok_or_else(|| "an action is implemented for a plain type name".to_owned())?;

    // The name the action's own `process` gives its input, if it names it: a single input is
    // the action's to speak for, and it already speaks for it where it is used.
    let named = item.items.iter().find_map(|item| match item {
        ImplItem::Fn(method) if method.sig.ident == ACTION_PROCESS => process_input_name(method),
        _ => None,
    });

    let mut id = None;
    let mut input = None;
    let mut output = None;
    for item in &item.items {
        match item {
            ImplItem::Const(associated) if associated.ident == ACTION_ID => {
                id = Some(action_id(&associated.expr)?);
            }
            ImplItem::Type(associated) => match associated.ident.to_string().as_str() {
                "Input" => input = Some((&associated.ty, &associated.attrs)),
                "Output" => output = Some(render_type(&associated.ty)?),
                _ => {}
            },
            _ => {}
        }
    }

    let id = id.ok_or_else(|| format!("`{type_name}` declares no `{ACTION_ID}`"))?;
    let (ty, attributes) = input.ok_or_else(|| format!("`{type_name}` declares no `Input`"))?;
    let (params, argument, call_args) = input_parameters(ty, attributes, named.as_deref())?;
    let output = output.ok_or_else(|| format!("`{type_name}` declares no `Output`"))?;

    Ok(Some(Action {
        id,
        type_name,
        params,
        argument,
        call_args,
        output,
    }))
}

/// How an action's input is handed to its entry points: the parameters to take, the value to
/// pass on, and the same value spelled as the arguments of a call.
///
/// A tuple of two or more is spread over one parameter per element, so a caller names each
/// part rather than building a tuple to hand over. A single value — and a one-element tuple,
/// which is one value wearing brackets — takes one parameter holding the whole input. The
/// names are the ones the input's doc comments hint at, then the action's own name for it
/// (`named`), and the fallbacks otherwise.
fn input_parameters(
    ty: &Type,
    attributes: &[Attribute],
    named: Option<&str>,
) -> Result<(String, String, String), String> {
    let hints = hint_names(attributes);

    if let Type::Tuple(tuple) = ty
        && tuple.elems.len() > 1
    {
        let mut parameters = Vec::new();
        let mut arguments = Vec::new();
        for (index, element) in tuple.elems.iter().enumerate() {
            let name = hints
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("p{index}"));

            parameters.push(format!("{name}: {}", render_type(element)?));
            arguments.push(name);
        }

        let call_args = arguments.join(", ");

        return Ok((parameters.join(", "), format!("({call_args})"), call_args));
    }

    let name = hints
        .first()
        .map(String::as_str)
        .or(named)
        .unwrap_or("input")
        .to_owned();

    Ok((format!("{name}: {}", render_type(ty)?), name.clone(), name))
}

/// The name an action's `process` gives its input, if it names it.
///
/// The first parameter is the input — the trait spells it so — and an action that names it
/// names it for a caller too. A parameter that is not the wrapped input, or not a plain name,
/// says nothing, and the fallback stands.
fn process_input_name(method: &ImplItemFn) -> Option<String> {
    let FnArg::Typed(parameter) = method.sig.inputs.first()? else {
        return None;
    };

    if !wraps_input(&parameter.ty) {
        return None;
    }

    let Pat::Ident(name) = parameter.pat.as_ref() else {
        return None;
    };

    Some(name.ident.to_string())
}

/// Whether `ty` is the input as the trait wraps it: `OnlyWorkspace<...>`.
fn wraps_input(ty: &Type) -> bool {
    let Type::Path(TypePath { path, .. }) = ty else {
        return false;
    };

    path.segments
        .last()
        .is_some_and(|segment| segment.ident == ONLY_WORKSPACE)
}

/// The parameter names an input's doc comments hint at, in order.
///
/// Only a list item says anything — a line that is `- name: Type` — and only the name half is
/// read: what a parameter *is* comes from the tuple itself, and a hint that disagreed with it
/// could only mislead. The prose around the list is ignored, so a description can be written
/// the usual way.
fn hint_names(attributes: &[Attribute]) -> Vec<String> {
    let mut names = Vec::new();

    for attribute in attributes {
        if !attribute.path().is_ident("doc") {
            continue;
        }

        let Meta::NameValue(name_value) = &attribute.meta else {
            continue;
        };
        let Expr::Lit(ExprLit {
            lit: Lit::Str(text),
            ..
        }) = &name_value.value
        else {
            continue;
        };

        for line in text.value().lines() {
            let Some(item) = line.trim().strip_prefix('-') else {
                continue;
            };
            let Some((name, _)) = item.split_once(':') else {
                continue;
            };

            let name = name.trim();
            if !name.is_empty() {
                names.push(name.to_owned());
            }
        }
    }

    names
}

/// The id an action declares, as the number the registry is laid out by.
///
/// Only a number is read: an id computed somewhere else is not something a registry can be
/// laid out by, and saying so beats laying one out around a guess.
fn action_id(expression: &Expr) -> Result<u32, String> {
    let Expr::Lit(ExprLit {
        lit: Lit::Int(literal),
        ..
    }) = expression
    else {
        return Err(format!("`{ACTION_ID}` must be a number"));
    };

    literal
        .base10_parse::<u32>()
        .map_err(|error| format!("`{ACTION_ID}` is not a `u32`: {error}"))
}

/// The name of a plain path type: no qualification, no generics.
fn plain_name(ty: &Type) -> Option<String> {
    let Type::Path(TypePath {
        qself: None, path, ..
    }) = ty
    else {
        return None;
    };

    let segment = path.segments.last()?;
    if path.segments.len() != 1 || !matches!(segment.arguments, PathArguments::None) {
        return None;
    }

    Some(segment.ident.to_string())
}

/// Renders a type the way it was written, so the generated signature can name it.
///
/// Only the shapes a signature realistically holds are understood; anything else is
/// reported rather than guessed at, since a wrong rendering would be a wrong signature.
fn render_type(ty: &Type) -> Result<String, String> {
    match ty {
        Type::Path(type_path) => {
            if type_path.qself.is_some() {
                return Err("a qualified path is not supported".to_owned());
            }

            render_path(&type_path.path)
        }
        Type::Reference(reference) => {
            let mutability = reference.mutability.map_or("", |_| "mut ");
            Ok(format!("&{mutability}{}", render_type(&reference.elem)?))
        }
        Type::Slice(slice) => Ok(format!("[{}]", render_type(&slice.elem)?)),
        Type::Tuple(tuple) => {
            let elements = tuple
                .elems
                .iter()
                .map(render_type)
                .collect::<Result<Vec<_>, _>>()?;

            if let [single] = elements.as_slice() {
                return Ok(format!("({single},)"));
            }

            Ok(format!("({})", elements.join(", ")))
        }
        _ => Err("this type shape is not supported".to_owned()),
    }
}

/// Renders a path, with whatever generic arguments its segments carry.
fn render_path(path: &syn::Path) -> Result<String, String> {
    let mut rendered = String::new();
    if path.leading_colon.is_some() {
        rendered.push_str("::");
    }

    for (index, segment) in path.segments.iter().enumerate() {
        if index > 0 {
            rendered.push_str("::");
        }
        rendered.push_str(&segment.ident.to_string());

        if matches!(segment.arguments, PathArguments::None) {
            continue;
        }

        let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
            return Err("parenthesized generic arguments are not supported".to_owned());
        };

        let arguments = arguments
            .args
            .iter()
            .map(|argument| match argument {
                GenericArgument::Type(ty) => render_type(ty),
                _ => Err("this generic argument is not supported".to_owned()),
            })
            .collect::<Result<Vec<_>, _>>()?;

        rendered.push('<');
        rendered.push_str(&arguments.join(", "));
        rendered.push('>');
    }

    Ok(rendered)
}

/// Renders one generated file from its template, with one arm per action.
///
/// Each arm is handed the action's four names, so the template decides where they go;
/// a template that cannot expand — its blocks do not pair up — stops the build rather
/// than writing out a file that is only half generated.
fn render(manifest: &Path, template: &str, actions: &[Action]) -> String {
    let path = manifest.join(template);
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => fail(&format!("reading {}: {error}", path.display())),
    };

    let mut template = Template::from(source);
    // Which slot an action takes depends on every other action's id, so the rows are laid
    // out here: one arm per action cannot say where a hole goes.
    template.insert_param("rows".to_owned(), registry_rows(actions));

    let arms = template.add_impl(ACTIONS_BLOCK.to_owned());
    for action in actions {
        arms.push(HashMap::from([
            ("type_name".to_owned(), action.type_name.clone()),
            ("stem".to_owned(), action.stem()),
            ("params".to_owned(), action.params.clone()),
            ("argument".to_owned(), action.argument.clone()),
            ("call_args".to_owned(), action.call_args.clone()),
            ("output".to_owned(), action.output.clone()),
        ]));
    }

    template.expand().map_or_else(
        || {
            fail(&format!(
                "expanding {}: its blocks do not pair up, or a block is nested",
                path.display()
            ))
        },
        |rendered| {
            // `expand` trims what it produces, so the newline a text file ends with is
            // put back here.
            rendered + "\n"
        },
    )
}

/// Renders the module that declares the action files, from its template.
///
/// It is what makes adding an action one file and nothing else: the declarations are read
/// off the directory rather than written by hand, so a file and its module cannot come
/// apart.
fn render_modules(manifest: &Path, modules: &[String]) -> String {
    let path = manifest.join(MODULES_TEMPLATE);
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => fail(&format!("reading {}: {error}", path.display())),
    };

    let mut template = Template::from(source);
    template.insert_param("modules".to_owned(), module_declarations(modules));

    template.expand().map_or_else(
        || {
            fail(&format!(
                "expanding {}: its blocks do not pair up, or a block is nested",
                path.display()
            ))
        },
        |rendered| rendered + "\n",
    )
}

/// What each action file needs said about it: its module, and what that module exports.
///
/// A module is named after its file, so a file is reached by the name it was written under
/// rather than by the action inside it.
fn module_declarations(modules: &[String]) -> String {
    modules
        .iter()
        .map(|module| format!("mod {module};\npub use {module}::*;"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Refuses two actions that claim one id.
///
/// The registry is laid out by id, so a repeated one would put an action where another
/// belongs. The build stops here, while both are still there to be seen.
fn ensure_unique_ids(actions: &[Action]) -> Result<(), String> {
    for (index, action) in actions.iter().enumerate() {
        if let Some(other) = actions[index + 1..]
            .iter()
            .find(|other| other.id == action.id)
        {
            return Err(format!(
                "`{}` and `{}` both claim id {}",
                action.type_name, other.type_name, action.id
            ));
        }
    }

    Ok(())
}

/// The registry's rows, laid out by id.
///
/// The slot at `id` is the action that answers to it; a slot no action claims is `None`, so
/// a hole in the ids stays a hole rather than shifting what comes after it.
fn registry_rows(actions: &[Action]) -> String {
    let Some(last) = actions.last() else {
        return String::new();
    };

    let mut rows = Vec::new();
    for id in 0..=last.id {
        match actions.iter().find(|action| action.id == id) {
            Some(action) => rows.push(format!(
                "        std::option::Option::Some(std::boxed::Box::new({})),",
                action.type_name
            )),
            None => rows.push("        std::option::Option::None,".to_owned()),
        }
    }

    rows.join("\n")
}

/// Turns a type name into the snake case its entry points are named after.
fn snake_case(name: &str) -> String {
    let mut rendered = String::new();
    for (index, character) in name.char_indices() {
        if character.is_uppercase() && index > 0 {
            rendered.push('_');
        }
        rendered.extend(character.to_lowercase());
    }

    rendered
}

/// Writes `contents` to `path`, unless it already says exactly that.
///
/// Rewriting a file that has not changed would give it a new timestamp, and the C header
/// generator watches the sources, so an unchanged file is left alone.
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
/// A build script can only speak to cargo through its directives, so the diagnostic is
/// forwarded line by line and the script then exits non-zero: a file that is only partly
/// generated must never be left behind as if it were whole.
fn fail(message: &str) -> ! {
    for line in message.lines() {
        println!("cargo:warning={line}");
    }

    exit(1);
}
