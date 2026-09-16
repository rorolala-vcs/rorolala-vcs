//! Generates the C header for Rorolala's `#[lazyffi]` surface.
//!
//! A generic generator cannot render `#[lazyffi]` output: the generated
//! signatures are written in terms of associated types (`<T as InputType>::From`),
//! which it has no way to resolve. This generator knows the `#[lazyffi]` rules
//! instead, so it works straight from the sources — no macro expansion, and no
//! nightly toolchain.
//!
//! The rules themselves come from `rorolala-utils-lazyffi-core`, the same crate
//! the attribute macro uses, so the two cannot drift apart.
//!
//! A type the generator cannot map to C is reported as an error pointing at the
//! offending line, never skipped: an incomplete header is worse than no header.
//!
//! # Tradeoff: the attribute is parsed twice
//!
//! `#[lazyffi]` itself, and the `export = <Name>` override in it, are parsed here
//! *and* in `rorolala-utils-lazyffi-macros`, and that duplication is deliberate.
//!
//! The two never see the same input. An attribute macro receives its item with the
//! `#[lazyffi]` attribute **already stripped**, so the macro can only read `export`
//! from the attribute's own argument tokens; this generator reads the sources, so
//! it sees `#[lazyffi(export = ...)]` in place and parses it out of the item's
//! attributes. There is no shared shape to factor out — only the *defaults* are,
//! and those do live in `rorolala-utils-lazyffi-core`.
//!
//! What must not drift is therefore the naming rule (`ffi_<snake>` /
//! `ffi_<type>_<method>` / `FFI<Pascal>` and its `Tag`/`Payload`/`<Variant>`
//! derivatives), the built-in repr table, and the shape rules (`variant_fields`,
//! `variant_needs_companion`), not this parser. If you add a second attribute
//! argument, or change when a variant needs a companion struct, you have to teach
//! both sides about it.
//!
//! # Definition order
//!
//! C needs a type to be complete before it is used by value, so the generated
//! definitions are emitted in dependency order rather than source order. A cycle
//! (which Rust cannot express by value either) is reported as an error.
//!
//! # Documentation
//!
//! Only two parts of an item's Rust docs reach the header, so C readers get a
//! summary instead of a Rust manual: the first line, and the section under a
//! `# FFI` heading (the heading itself is dropped, and the section ends at the
//! next heading). Generated machinery — the tag enum, the payload union, the
//! companion structs — carries no comment of its own; what belongs to a variant
//! or a field is documented on the member that mirrors it, since that is where a
//! C reader looks.
//!
//! # Name resolution
//!
//! Types are resolved by their **last path segment**, so an export in one crate can
//! mention a type defined in a sibling crate (`auth::Token`, `rorolala_auth::Token`
//! and a bare `Token` all name the same repr). That is deliberately weaker than
//! name resolution: an aliased import does not resolve, and two types sharing a
//! name are ambiguous. Both are reported as errors rather than guessed at, and two
//! definitions that would claim the same C name — the usual symptom of such a
//! clash — are rejected outright.

#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error as StdError;
use std::fmt::{self, Write as _};
use std::fs;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use annotate_snippets::{AnnotationKind, Level, Renderer, Snippet, renderer::DecorStyle};
use quote::quote;
use rorolala_utils_lazyffi_core::{
    FREE_STRING, RESULT_ERR_VARIANT, RESULT_OK_VARIANT, RESULT_REPR, STRING_REPR, VariantFields,
    c_variant_name, for_each_scalar, free_name, method_name, payload_type_name, tag_type_name,
    type_name, value_name, variant_needs_companion, variant_type_name,
};
use syn::{
    Attribute, Expr, ExprLit, Fields, FnArg, GenericArgument, ImplItem, Item, ItemConst, ItemEnum,
    ItemFn, ItemImpl, ItemStruct, Lit, Meta, Pat, PathArguments, ReceiverKind, ReturnType,
    Signature, Type, spanned::Spanned,
};

/// File name of the generated header.
pub const HEADER_NAME: &str = "rorolala_ffi.h";

/// Renders the scalar types handed over by `for_each_scalar!` as a name array.
macro_rules! scalar_names {
    ($($scalar:ident),* $(,)?) => {
        &[$(stringify!($scalar)),*]
    };
}

/// Names of the scalar types whose repr is themselves.
///
/// Taken from `lazyffi-core`, so this list cannot drift from the conversions
/// `builtin` actually implements.
const SCALAR_NAMES: &[&str] = for_each_scalar!(scalar_names);

/// Include guard used in the generated header.
const INCLUDE_GUARD: &str = "ROROLALA_FFI_H";

/// C spelling of a string in a position Rust only reads.
///
/// A parameter is always such a position: the callee copies out of the C string and
/// the caller keeps its buffer, so the string is spelled `const` — which is also what
/// lets a C++ caller pass a string literal.
const READ_ONLY_STRING: &str = "const char *";

/// Documentation heading whose section is carried into the header.
const FFI_HEADING: &str = "# FFI";

/// Comment written at the top of every generated header.
const BANNER: &str = "\
/*!
 * This file is automatically generated by rorolala-dev-bindgen.
 * DO NOT EDIT THIS FILE MANUALLY.
 *
 * All string-returning functions allocate memory that must be freed using
 * free_string().
 */
";

/// Header preamble: the include guard, the standard headers the reprs need, the
/// `extern "C"` block a C++ consumer needs, and the declaration of the string
/// release helper.
///
/// The helper is declared unconditionally: it costs nothing, and a C caller could
/// not release a returned string otherwise.
fn preamble() -> String {
    let mut out = format!(
        "\n#ifndef {INCLUDE_GUARD}\n#define {INCLUDE_GUARD}\n\n\
         #include <stdarg.h>\n#include <stdbool.h>\n#include <stdint.h>\n#include <stdlib.h>\n\n\
         #ifdef __cplusplus\nextern \"C\" {{\n#endif\n\n\
         void {FREE_STRING}(char *string);\n\n"
    );

    out.push_str(&result_declarations());

    out
}

/// The result type every header declares, whether or not anything uses it.
///
/// A `Result<T, E>` return cannot cross as a repr generated for it — one C layout
/// cannot name two payload types, and a type per `(T, E)` pair would collide the moment
/// two exports returned the same one. So there is one result type for the whole
/// surface, declared once here the way the string release is, and the layout below is
/// the contract `rorolala-utils-lazyffi` implements.
fn result_declarations() -> String {
    let repr = RESULT_REPR;
    let tag = tag_type_name(repr);
    let ok = c_variant_name(repr, RESULT_OK_VARIANT);
    let error = c_variant_name(repr, RESULT_ERR_VARIANT);

    let mut out = String::new();
    let _ = writeln!(out, "/** Which side of a `Result` a `{repr}` carries. */");
    let _ = writeln!(out, "typedef enum {tag} {{");
    let _ = writeln!(
        out,
        "  /** The call succeeded, and the payload is its value. */"
    );
    let _ = writeln!(out, "  {ok} = 0,");
    let _ = writeln!(
        out,
        "  /** The call failed, and the payload is the error. */"
    );
    let _ = writeln!(out, "  {error} = 1,");
    let _ = writeln!(out, "}} {tag};\n");
    let _ = writeln!(out, "/**");
    let _ = writeln!(out, " * What a fallible export hands back.");
    let _ = writeln!(out, " *");
    let _ = writeln!(
        out,
        " * The payload is owned by the caller: read the tag to learn which side it is,"
    );
    let _ = writeln!(
        out,
        " * cast it to the type the function names for that side, and release it with"
    );
    let _ = writeln!(
        out,
        " * that type's own `free_*`. It is null when there is nothing to carry."
    );
    let _ = writeln!(out, " */");
    let _ = writeln!(out, "typedef struct {repr} {{");
    let _ = writeln!(out, "  {tag} tag;");
    let _ = writeln!(out, "  void * payload;");
    let _ = writeln!(out, "}} {repr};\n");

    out
}

/// Header footer: closes the `extern "C"` block and the include guard.
fn footer() -> String {
    format!("#ifdef __cplusplus\n}}\n#endif\n\n#endif  /* {INCLUDE_GUARD} */\n")
}

/// Everything one generation run needs.
#[derive(Debug)]
pub struct Config<'a> {
    /// Directories scanned recursively for `#[lazyffi]` items.
    pub source_roots: &'a [PathBuf],
    /// Directory the header is written into; created when missing.
    pub output_dir: &'a Path,
}

/// Something in the sources that cannot be rendered into the header.
#[derive(Debug)]
pub struct Diagnostic {
    /// Source file the item lives in.
    pub path: PathBuf,
    /// Full text of that file, kept for the diagnostic.
    pub source: String,
    /// 1-based line the item starts on.
    pub line: usize,
    /// Name of the item being rendered.
    pub item: String,
    /// What exactly is wrong with it.
    pub label: String,
    /// Extra context shown below the snippet, when the label cannot carry it.
    pub note: Option<String>,
}

/// What can go wrong while generating the header.
#[derive(Debug)]
pub enum Error {
    /// A source file could not be read, or the header could not be written.
    Io {
        /// The offending path.
        path: PathBuf,
        /// The underlying error.
        source: io::Error,
    },
    /// A source file could not be parsed as Rust.
    Parse {
        /// The offending path.
        path: PathBuf,
        /// The underlying error.
        source: syn::Error,
    },
    /// Items were found that cannot be rendered into C.
    Diagnostics(Vec<Diagnostic>),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Parse { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Diagnostics(diagnostics) => formatter.write_str(&render_diagnostics(diagnostics)),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Diagnostics(_) => None,
        }
    }
}

/// Generates the header and returns the path it was written to.
///
/// # Errors
///
/// Fails when a source file cannot be read or parsed, when an item uses a type
/// that has no repr-C sibling, or when the header cannot be written into
/// [`Config::output_dir`].
pub fn generate(config: &Config<'_>) -> Result<PathBuf, Error> {
    let mut exports = Exports::default();
    for root in config.source_roots {
        scan_dir(root, &mut exports)?;
    }

    let header = render(&exports).map_err(Error::Diagnostics)?;

    fs::create_dir_all(config.output_dir).map_err(|source| Error::Io {
        path: config.output_dir.to_path_buf(),
        source,
    })?;

    let path = config.output_dir.join(HEADER_NAME);
    fs::write(&path, header).map_err(|source| Error::Io {
        path: path.clone(),
        source,
    })?;

    Ok(path)
}

/// Where an item was read from, kept so diagnostics can point back at it.
#[derive(Clone, Debug)]
struct Origin {
    /// Source file.
    path: PathBuf,
    /// Full text of that file.
    source: Arc<String>,
}

/// An exported type, in either of the two shapes `#[lazyffi]` supports.
enum TypeItem {
    /// An exported struct.
    Struct(ItemStruct),
    /// An exported enum.
    Enum(ItemEnum),
}

/// The `#[lazyffi]` items collected from the sources.
#[derive(Default)]
struct Exports {
    /// Exported constants.
    constants: Vec<(ItemConst, Origin)>,
    /// Exported structs and enums.
    types: Vec<(TypeItem, Origin)>,
    /// Exported free functions.
    functions: Vec<(ItemFn, Origin)>,
    /// Exported `impl` blocks.
    impls: Vec<(ItemImpl, Origin)>,
}

/// Scans `dir` recursively, collecting `#[lazyffi]` items.
fn scan_dir(dir: &Path, exports: &mut Exports) -> Result<(), Error> {
    let entries = fs::read_dir(dir).map_err(|source| Error::Io {
        path: dir.to_path_buf(),
        source,
    })?;

    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| Error::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        paths.push(entry.path());
    }
    paths.sort();

    for path in paths {
        if path.is_dir() {
            scan_dir(&path, exports)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            scan_file(&path, exports)?;
        }
    }

    Ok(())
}

/// Parses one source file and collects its `#[lazyffi]` items.
fn scan_file(path: &Path, exports: &mut Exports) -> Result<(), Error> {
    let source = fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let file = syn::parse_file(&source).map_err(|source| Error::Parse {
        path: path.to_path_buf(),
        source,
    })?;

    let origin = Origin {
        path: path.to_path_buf(),
        source: Arc::new(source),
    };
    collect_items(&file.items, &origin, exports);
    Ok(())
}

/// Collects `#[lazyffi]` items, descending into inline modules.
fn collect_items(items: &[Item], origin: &Origin, exports: &mut Exports) {
    for item in items {
        match item {
            Item::Mod(module) => {
                if let Some((_, inner)) = &module.content {
                    collect_items(inner, origin, exports);
                }
            }
            Item::Const(item) if has_lazyffi(&item.attrs) => {
                exports.constants.push((item.clone(), origin.clone()));
            }
            Item::Struct(item) if has_lazyffi(&item.attrs) => {
                exports
                    .types
                    .push((TypeItem::Struct(item.clone()), origin.clone()));
            }
            Item::Enum(item) if has_lazyffi(&item.attrs) => {
                exports
                    .types
                    .push((TypeItem::Enum(item.clone()), origin.clone()));
            }
            Item::Fn(item) if has_lazyffi(&item.attrs) => {
                exports.functions.push((item.clone(), origin.clone()));
            }
            Item::Impl(item) if has_lazyffi(&item.attrs) => {
                exports.impls.push((item.clone(), origin.clone()));
            }
            _ => {}
        }
    }
}

/// Whether an item carries `#[lazyffi]`, in any of its spellings.
fn has_lazyffi(attrs: &[Attribute]) -> bool {
    attrs.iter().any(is_lazyffi)
}

/// Whether one attribute is `#[lazyffi]`, in any of its spellings.
fn is_lazyffi(attr: &Attribute) -> bool {
    attr.path()
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "lazyffi")
}

/// Reads `export = <Name>` from the `#[lazyffi]` attribute, if present.
///
/// This parser is a deliberate second copy of the one in
/// `rorolala-utils-lazyffi-macros`; see the crate docs for why the two cannot be
/// merged, and mind both when adding attribute arguments.
fn export_override(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs.iter().filter(|attr| is_lazyffi(attr)) {
        let mut name = None;
        let _ = attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("export") {
                return Ok(());
            }
            let value = meta.value()?;
            if let Ok(literal) = value.parse::<syn::LitStr>() {
                name = Some(literal.value());
            } else if let Ok(ident) = value.parse::<syn::Ident>() {
                name = Some(ident.to_string());
            } else {
                return Err(meta.error("expected an identifier or a string literal"));
            }
            Ok(())
        });
        if name.is_some() {
            return name;
        }
    }

    None
}

/// The repr-C siblings of the exported types, plus every C name the header
/// defines.
struct Reprs {
    /// Every declaration of each exported name: the file it was written in, and the
    /// repr-C name it is exported under.
    ///
    /// A list rather than one entry per name, because a name is not a declaration: two
    /// modules can each declare a `Config` and export it under a name of its own. Which
    /// one is meant is decided by where the name is written — see [`Reprs::nearest`] —
    /// and which one a declaration is rendered as is decided by
    /// [`Reprs::declared_in`].
    claims: BTreeMap<String, Vec<(PathBuf, String)>>,
    /// Every type name the header defines.
    names: BTreeSet<String>,
    /// The release each type was given, keyed by its repr-C name.
    ///
    /// Kept for the same reason the repr is: the release mirrors the name C knows the
    /// type by, which is its `export` where it has one — see [`release_name`] — and a
    /// signature that hands one out has to name the release that matches.
    releases: BTreeMap<String, String>,
    /// Repr-C names of the types that are opaque to C.
    ///
    /// An exported `struct` is one: C is told that the type exists and nothing else
    /// about it, so it can never hold a value of one, and every appearance of one in
    /// the header is a pointer.
    opaque: BTreeSet<String>,
}

impl Reprs {
    /// Records that `name` is declared in `file` under `repr`.
    ///
    /// A built-in has no file of its own, and is recorded with an empty one: it is
    /// then further from every source file than any declaration in the tree, which is
    /// what a built-in should be.
    fn claim(&mut self, name: &str, file: &Path, repr: &str) {
        self.claims
            .entry(name.to_string())
            .or_default()
            .push((file.to_path_buf(), repr.to_string()));
    }

    /// The repr of the declaration of `name` written in `file` itself.
    ///
    /// This is how a definition is rendered: a `struct Config` is exported under the
    /// name its own `#[lazyffi]` gave it, never under the name a table of bare names
    /// happens to hold.
    fn declared_in(&self, name: &str, file: &Path) -> Option<String> {
        self.claims
            .get(name)?
            .iter()
            .find(|(claimed, _)| claimed == file)
            .map(|(_, repr)| repr.clone())
    }

    /// The repr `name` means where it is written.
    ///
    /// The nearest declaration wins, because that is the one a bare name means:
    /// `modules/vault/src/init.rs` saying `CreationError` means the one beside it, not
    /// the one `modules/workspace` declares. Two declarations equally near are not
    /// resolved at all, and the caller reports it rather than picking one.
    fn nearest(&self, name: &str, file: &Path) -> Option<String> {
        let mut ranked: Vec<(usize, &str)> = self
            .claims
            .get(name)?
            .iter()
            .map(|(claimed, repr)| (shared_depth(claimed, file), repr.as_str()))
            .collect();
        ranked.sort_by_key(|(depth, _)| std::cmp::Reverse(*depth));

        match ranked.as_slice() {
            [(depth, _), (next, _), ..] if depth == next => None,
            [(_, repr), ..] => Some((*repr).to_owned()),
            [] => None,
        }
    }

    /// The C spelling of a value whose repr-C name is `repr`.
    fn shape(&self, repr: &str) -> String {
        if self.opaque.contains(repr) {
            format!("*mut {repr}")
        } else {
            repr.to_string()
        }
    }
}

/// The name of the function that releases a value of the type `attrs` describe.
///
/// The name mirrors the one C knows the type by: its `export` where it has one, and the
/// type's own Rust name where it does not. The Rust name alone would give two types the
/// same release, which is what two modules each keeping a `Config` amounts to; the
/// `export` is what tells them apart, so it is what the release follows.
fn release_name(rust_name: &str, attrs: &[Attribute]) -> String {
    free_name(&export_override(attrs).unwrap_or_else(|| rust_name.to_string()))
}

/// How many leading components two paths have in common.
///
/// The measure of how near one declaration is to a reference: the more of a path two
/// files share, the closer they are in the tree.
fn shared_depth(one: &Path, other: &Path) -> usize {
    one.components()
        .zip(other.components())
        .take_while(|(one, other)| one == other)
        .count()
}

/// Builds the type tables.
///
/// The built-in types come from `lazyffi-core`, so they match what `builtin`
/// actually implements; every `#[lazyffi]` type maps to the repr its invocation
/// generates, including the parts of a data-carrying enum. An exported `struct` is
/// also recorded as opaque, since its repr is never spelled out in C.
fn reprs(exports: &Exports) -> Reprs {
    let mut reprs = Reprs {
        claims: BTreeMap::new(),
        names: BTreeSet::new(),
        releases: BTreeMap::new(),
        opaque: BTreeSet::new(),
    };

    for scalar in SCALAR_NAMES {
        reprs.claim(scalar, Path::new(""), scalar);
    }
    reprs.claim("String", Path::new(""), STRING_REPR);
    reprs.claim("PathBuf", Path::new(""), STRING_REPR);
    // `Path` is unsized and can only appear behind a reference, but registering it
    // means `&Path` resolves and `&mut Path` is rejected the same way a string is.
    reprs.claim("Path", Path::new(""), STRING_REPR);
    // The same, for the borrowed spelling of a string.
    reprs.claim("str", Path::new(""), STRING_REPR);

    for (item, origin) in &exports.types {
        let (rust_name, attrs) = match item {
            TypeItem::Struct(item) => (item.ident.to_string(), &item.attrs),
            TypeItem::Enum(item) => (item.ident.to_string(), &item.attrs),
        };

        let default = type_name(&rust_name);
        let repr = export_override(attrs).unwrap_or(default);
        reprs.claim(&rust_name, &origin.path, &repr);
        reprs.names.insert(repr.clone());
        reprs
            .releases
            .insert(repr.clone(), release_name(&rust_name, attrs));

        if matches!(item, TypeItem::Struct(_)) {
            reprs.opaque.insert(repr.clone());
        }

        if let TypeItem::Enum(item) = item
            && carries_data(item)
        {
            reprs.names.insert(tag_type_name(&repr));
            reprs.names.insert(payload_type_name(&repr));
            for variant in &item.variants {
                if variant_needs_companion(&variant_fields(&variant.fields)) {
                    reprs
                        .names
                        .insert(variant_type_name(&repr, &variant.ident.to_string()));
                }
            }
        }
    }

    reprs
}

/// Resolves the repr-C sibling of a Rust type.
///
/// Resolution is by the **last path segment**, not by full path: a signature saying
/// `auth::Token`, `rorolala_auth::Token` or a bare `Token` all name the same repr.
/// Without that, an export in one crate could not mention a type defined in a
/// sibling crate, which is the normal shape of this workspace.
///
/// What the segment does not say is *which* declaration is meant when two of them
/// share a name — `vault` and `workspace` each declare a `CreationError`. That is
/// decided by where the name is written, the nearest declaration winning; see
/// [`Reprs::nearest`].
///
/// Two consequences worth knowing:
///
/// - an alias (`use rorolala_auth::Token as Seal;`) does not resolve, because no
///   `Seal` is defined here — it is reported, not skipped;
/// - two declarations that are equally near a name resolve to neither, and the
///   reference is reported rather than guessed at.
///
/// `Self` has no entry in any table either — it only appears inside an `impl`,
/// where it stands for the target type; [`Resolution::self_repr`] carries that.
fn repr_of(ty: &Type, resolution: &Resolution<'_>) -> Option<String> {
    if let Type::Path(path) = ty
        && path.qself.is_none()
        && let Some(segment) = path.path.segments.last()
        && segment.arguments.is_none()
    {
        let ident = segment.ident.to_string();
        if ident == "Self" {
            return resolution.self_repr.map(ToString::to_string);
        }
        return resolution.reprs.nearest(&ident, resolution.origin);
    }

    None
}

/// What leads from a Rust type to its C spelling.
struct Resolution<'a> {
    /// The repr-C siblings of the exported types.
    reprs: &'a Reprs,
    /// The file the name being resolved was written in.
    ///
    /// A name can be declared by more than one module, and which one is meant is
    /// decided by how near it is to where the name is written.
    origin: &'a Path,
    /// The repr of `Self`, when rendering inside an `impl`.
    self_repr: Option<&'a str>,
}

impl<'a> Resolution<'a> {
    /// Resolves types written in `origin`, outside an `impl`, where `Self` cannot
    /// appear.
    const fn plain(reprs: &'a Reprs, origin: &'a Path) -> Self {
        Self {
            reprs,
            origin,
            self_repr: None,
        }
    }
}

/// C spelling of a repr type.
fn c_type(repr: &str, reprs: &Reprs) -> Option<String> {
    if let Some(scalar) = scalar_c_type(repr) {
        return Some(scalar.to_string());
    }

    if repr == STRING_REPR {
        return Some("char *".to_string());
    }

    if let Some(inner) = repr.strip_prefix("*mut ") {
        return Some(format!("{} *", c_type(inner, reprs)?));
    }

    // A generated repr type keeps its Rust name in C.
    if reprs.names.contains(repr) {
        return Some(repr.to_string());
    }

    None
}

/// C spelling of the scalar types whose repr is themselves.
fn scalar_c_type(rust: &str) -> Option<&'static str> {
    Some(match rust {
        "bool" => "bool",
        // A Rust `char` is a 32-bit scalar, so it shares `u32`'s C spelling.
        "char" | "u32" => "uint32_t",
        "f32" => "float",
        "f64" => "double",
        "i8" => "int8_t",
        "i16" => "int16_t",
        "i32" => "int32_t",
        "i64" => "int64_t",
        "i128" => "__int128",
        "isize" => "intptr_t",
        "u8" => "uint8_t",
        "u16" => "uint16_t",
        "u64" => "uint64_t",
        "u128" => "unsigned __int128",
        "usize" => "uintptr_t",
        _ => return None,
    })
}

/// Whether a type is a reference to `str`.
fn is_str_ref(ty: &Type) -> bool {
    if let Type::Reference(reference) = ty
        && let Type::Path(inner) = &*reference.elem
    {
        return inner.path.is_ident("str");
    }

    false
}

/// Maps a variant's fields onto the shape `lazyffi-core` reasons about.
fn variant_fields(fields: &Fields) -> VariantFields {
    match fields {
        Fields::Unit => VariantFields::Unit,
        Fields::Unnamed(unnamed) => VariantFields::Unnamed(unnamed.unnamed.len()),
        Fields::Named(named) => VariantFields::Named(named.named.len()),
    }
}

/// Whether an enum has any variant carrying data.
fn carries_data(item: &ItemEnum) -> bool {
    item.variants
        .iter()
        .any(|variant| !matches!(variant.fields, Fields::Unit))
}

/// The documentation of one field of a variant, by the name it is known under in
/// C (`_0` for tuple fields).
fn field_docs(variant: &syn::Variant, field_name: &str) -> Vec<String> {
    let Fields::Named(named) = &variant.fields else {
        return Vec::new();
    };

    named
        .named
        .iter()
        .find(|field| field.ident.as_ref().is_some_and(|name| name == field_name))
        .map_or_else(Vec::new, |field| doc_lines(&field.attrs))
}

/// The name/type pairs of a variant's payload, sharing the macro's naming: tuple
/// fields are `_0`, `_1`, … and braced fields keep their names.
fn payload_fields(fields: &Fields) -> Vec<(String, &Type)> {
    match fields {
        Fields::Unit => Vec::new(),
        Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .enumerate()
            .map(|(index, field)| (format!("_{index}"), &field.ty))
            .collect(),
        Fields::Named(named) => named
            .named
            .iter()
            .filter_map(|field| {
                let ident = field.ident.as_ref()?;
                Some((ident.to_string(), &field.ty))
            })
            .collect(),
    }
}

/// One C type definition in the header.
#[derive(Clone)]
struct Definition {
    /// C name of the defined type.
    name: String,
    /// Rendered C definition.
    text: String,
    /// Where it came from, for diagnostics.
    origin: Origin,
    /// Name of the item being rendered, for diagnostics.
    item: String,
    /// 1-based line of the item, for diagnostics.
    line: usize,
}

/// Where a rendering pass sends the problems it finds.
struct Reporting<'a> {
    /// File the item came from, for the diagnostic.
    origin: &'a Origin,
    /// Diagnostics collected so far.
    diagnostics: &'a mut Vec<Diagnostic>,
}

impl<'a> Reporting<'a> {
    /// Borrows the origin and the diagnostic sink for one rendering pass.
    const fn new(origin: &'a Origin, diagnostics: &'a mut Vec<Diagnostic>) -> Self {
        Self {
            origin,
            diagnostics,
        }
    }

    /// Records that something in `item` cannot be rendered.
    fn report(&mut self, line: usize, item: &str, label: String) {
        push_diagnostic(self.diagnostics, self.origin, line, item, label);
    }
}

/// Records that `item` cannot be rendered, on `line`.
fn push_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    origin: &Origin,
    line: usize,
    item: &str,
    label: String,
) {
    diagnostics.push(Diagnostic {
        path: origin.path.clone(),
        source: origin.source.as_ref().clone(),
        line,
        item: item.to_string(),
        label,
        note: None,
    });
}

/// `ty` with the padding `quote!` adds to paths removed.
fn compact(ty: &Type) -> String {
    quote!(#ty).to_string().replace(" :: ", "::")
}

/// The raw text of an item's `#[doc = ...]` attributes, one entry per line.
///
/// A `///` comment becomes exactly one such attribute per line, so this is the
/// documentation as written. `#[doc = include_str!(...)]` cannot be read without
/// expanding it, and is left out rather than guessed at.
fn doc_attribute_lines(attrs: &[Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| match &attr.meta {
            Meta::NameValue(name_value) => match &name_value.value {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(text),
                    ..
                }) => Some(text.value()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// The documentation lines that belong in the header.
///
/// Rust docs are aimed at Rust readers, so only two parts of them travel:
///
/// - the first line, which is the summary;
/// - the section under a `# FFI` heading, which is written for the C side on
///   purpose. The heading itself is dropped, and the section ends at the next
///   heading, so a `# Safety` or `# Panics` section stays on the Rust side.
///
/// When both are present they are separated by a blank line.
fn doc_lines(attrs: &[Attribute]) -> Vec<String> {
    let lines = doc_attribute_lines(attrs);
    let heading = lines.iter().position(|line| line.trim() == FFI_HEADING);
    let mut selected = Vec::new();

    if let Some(first) = lines.first() {
        let first = first.trim();
        if !first.is_empty() && heading != Some(0) {
            selected.push(first.to_string());
        }
    }

    if let Some(position) = heading {
        let mut section = lines
            .iter()
            .skip(position + 1)
            .take_while(|line| !is_doc_heading(line))
            .map(|line| doc_line_text(line))
            .collect::<Vec<_>>();

        while section.last().is_some_and(String::is_empty) {
            section.pop();
        }
        while section.first().is_some_and(String::is_empty) {
            section.remove(0);
        }

        if !section.is_empty() && !selected.is_empty() {
            selected.push(String::new());
        }
        selected.extend(section);
    }

    selected
}

/// Whether a documentation line opens a section.
fn is_doc_heading(line: &str) -> bool {
    line.trim_start().starts_with("# ")
}

/// One line of documentation with its marker removed.
///
/// Exactly one leading space goes away — the one a `///` comment is written with.
/// Anything beyond that is deliberate indentation and is kept.
fn doc_line_text(line: &str) -> String {
    let line = line.trim_end();
    line.strip_prefix(' ').unwrap_or(line).to_string()
}

/// Renders documentation lines as a C comment block, or nothing when there is
/// none.
fn render_doc(lines: &[String], indent: &str) -> String {
    match lines {
        [] => String::new(),
        [single] => format!("{indent}/** {} */\n", escape_comment(single)),
        _ => {
            let mut out = format!("{indent}/**\n");
            for line in lines {
                let text = escape_comment(line);
                if text.is_empty() {
                    let _ = writeln!(out, "{indent} *");
                } else {
                    let _ = writeln!(out, "{indent} * {text}");
                }
            }
            let _ = writeln!(out, "{indent} */");
            out
        }
    }
}

/// Neutralises a `*/` inside documentation so it cannot close the comment early.
fn escape_comment(text: &str) -> String {
    text.replace("*/", "* /")
}

/// The C spelling of a value of `ty`, as it appears in a declaration.
///
/// An opaque type has no value C could name, so this is a pointer to it.
fn shape_of(ty: &Type, resolution: &Resolution<'_>) -> Option<String> {
    let repr = repr_of(ty, resolution)?;
    c_type(&resolution.reprs.shape(&repr), resolution.reprs)
}

/// The C name of what a `&mut` to `ty` points at.
///
/// This is the repr itself rather than its shape: an opaque type is already a pointer
/// in a value position, but a `&mut` to it points at the handle, not at a pointer to a
/// handle.
fn pointee_of(ty: &Type, resolution: &Resolution<'_>) -> Option<String> {
    let repr = repr_of(ty, resolution)?;
    c_type(&repr, resolution.reprs)
}

/// Records that `ty` has no repr-C sibling, if that is what happened.
fn report_missing(
    spelling: Option<String>,
    ty: &Type,
    reporting: &mut Reporting<'_>,
    item: &str,
) -> Option<String> {
    if spelling.is_none() {
        reporting.report(
            ty.span().start().line,
            item,
            format!("`{}` has no repr-C sibling", compact(ty)),
        );
    }
    spelling
}

/// Whether a parameter is a string Rust only reads, whatever shape it arrives in.
///
/// That is a `String` or `PathBuf` by value — the callee copies out of the C string
/// and the caller keeps its buffer — and a borrowed `&str` or `&Path`, which is the
/// same thing with the copy made explicit.
fn read_only_string(ty: &Type, resolution: &Resolution<'_>) -> bool {
    let repr = match ty {
        Type::Reference(reference) if reference.mutability.is_none() => {
            repr_of(&reference.elem, resolution)
        }
        Type::Reference(_) => None,
        other => repr_of(other, resolution),
    };

    repr.as_deref() == Some(STRING_REPR)
}

/// The C spelling of a field or parameter of type `ty`.
fn field_c_type(
    ty: &Type,
    resolution: &Resolution<'_>,
    reporting: &mut Reporting<'_>,
    item: &str,
) -> Option<String> {
    report_missing(shape_of(ty, resolution), ty, reporting, item)
}

/// The C spelling of the type behind a reference.
fn pointee_c_type(
    ty: &Type,
    resolution: &Resolution<'_>,
    reporting: &mut Reporting<'_>,
    item: &str,
) -> Option<String> {
    report_missing(pointee_of(ty, resolution), ty, reporting, item)
}

/// Builds one definition per C type the exported types need.
fn build_definitions(
    exports: &Exports,
    reprs: &Reprs,
    definitions: &mut Vec<Definition>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (item, origin) in &exports.types {
        match item {
            TypeItem::Struct(item) => build_struct(item, origin, reprs, definitions),
            TypeItem::Enum(item) => build_enum(item, origin, reprs, definitions, diagnostics),
        }
    }
}

/// Builds the definition of one exported struct.
///
/// The struct is opaque: its fields are not converted and never reach C, so what is
/// emitted is an incomplete type plus the declaration of the release that frees the
/// handles exports hand out.
fn build_struct(
    item: &ItemStruct,
    origin: &Origin,
    reprs: &Reprs,
    definitions: &mut Vec<Definition>,
) {
    let rust_name = item.ident.to_string();
    let Some(repr) = reprs.declared_in(&rust_name, &origin.path) else {
        return;
    };
    let release = release_name(&rust_name, &item.attrs);

    definitions.push(Definition {
        text: format!(
            "{doc}typedef struct {repr} {repr};\n\nvoid {release}({repr} *value);\n\n",
            doc = render_doc(&doc_lines(&item.attrs), "")
        ),
        name: repr,
        origin: origin.clone(),
        item: rust_name,
        line: item.ident.span().start().line,
    });
}

/// Builds the definitions of one exported enum.
fn build_enum(
    item: &ItemEnum,
    origin: &Origin,
    reprs: &Reprs,
    definitions: &mut Vec<Definition>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let rust_name = item.ident.to_string();
    let Some(repr) = reprs.declared_in(&rust_name, &origin.path) else {
        return;
    };
    let line = item.ident.span().start().line;

    if !carries_data(item) {
        let release = release_name(&rust_name, &item.attrs);
        definitions.push(Definition {
            text: format!(
                "{doc}{}void {release}({repr} *value);\n\n",
                render_enum(&repr, &repr, item),
                doc = render_doc(&doc_lines(&item.attrs), "")
            ),
            name: repr,
            origin: origin.clone(),
            item: rust_name,
            line,
        });
        return;
    }

    let tag = tag_type_name(&repr);
    let payload = payload_type_name(&repr);

    // The tag and the payload are generated machinery, so they carry no comment
    // of their own; what a C reader wants is on their members, which come from
    // the variant documentation.
    definitions.push(Definition {
        text: render_enum(&tag, &repr, item),
        name: tag.clone(),
        origin: origin.clone(),
        item: rust_name.clone(),
        line,
    });

    for variant in &item.variants {
        build_companion(variant, &repr, origin, reprs, definitions, diagnostics);
    }

    definitions.push(Definition {
        text: render_payload(
            &payload,
            &repr,
            item,
            reprs,
            origin,
            diagnostics,
            &rust_name,
        ),
        name: payload.clone(),
        origin: origin.clone(),
        item: rust_name.clone(),
        line,
    });

    definitions.push(Definition {
        text: format!(
            "{doc}typedef struct {repr} {{\n  {tag} tag;\n  {payload} payload;\n}} {repr};\n\n\
             void {release}({repr} *value);\n\n",
            doc = render_doc(&doc_lines(&item.attrs), ""),
            release = release_name(&rust_name, &item.attrs)
        ),
        name: repr,
        origin: origin.clone(),
        item: rust_name,
        line,
    });
}

/// Renders a C enum, naming its enumerators after `repr`.
///
/// The numbers match the macro's tag enum: both count from zero in declaration
/// order and never look at the Rust discriminants. Each enumerator carries the
/// documentation of the variant it mirrors.
fn render_enum(name: &str, repr: &str, item: &ItemEnum) -> String {
    let variants = item
        .variants
        .iter()
        .enumerate()
        .map(|(index, variant)| {
            format!(
                "{doc}  {} = {index},",
                c_variant_name(repr, &variant.ident.to_string()),
                doc = render_doc(&doc_lines(&variant.attrs), "  ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("typedef enum {name} {{\n{variants}\n}} {name};\n\n")
}

/// Builds the companion struct of a variant whose payload needs one.
fn build_companion(
    variant: &syn::Variant,
    repr: &str,
    origin: &Origin,
    reprs: &Reprs,
    definitions: &mut Vec<Definition>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !variant_needs_companion(&variant_fields(&variant.fields)) {
        return;
    }

    let name = variant_type_name(repr, &variant.ident.to_string());
    let mut reporting = Reporting::new(origin, diagnostics);
    let resolution = Resolution::plain(reprs, &origin.path);
    let mut body = String::new();
    for (field_name, ty) in payload_fields(&variant.fields) {
        let label = format!("{repr}::{}.{field_name}", variant.ident);
        let Some(c_type) = field_c_type(ty, &resolution, &mut reporting, &label) else {
            continue;
        };
        body.push_str(&render_doc(&field_docs(variant, &field_name), "  "));
        let _ = writeln!(body, "  {c_type} {field_name};");
    }

    definitions.push(Definition {
        text: format!("typedef struct {name} {{\n{body}}} {name};\n\n"),
        name,
        origin: origin.clone(),
        item: repr.to_string(),
        line: variant.ident.span().start().line,
    });
}

/// Renders the payload union of a data-carrying enum.
///
/// The zero-sized slot the macro uses for unit variants is left out: it has no
/// bearing on the union's layout, and C has nothing to name there.
fn render_payload(
    name: &str,
    repr: &str,
    item: &ItemEnum,
    reprs: &Reprs,
    origin: &Origin,
    diagnostics: &mut Vec<Diagnostic>,
    rust_name: &str,
) -> String {
    let mut reporting = Reporting::new(origin, diagnostics);
    let resolution = Resolution::plain(reprs, &origin.path);
    let mut body = String::new();
    for variant in &item.variants {
        if matches!(variant.fields, Fields::Unit) {
            continue;
        }

        let variant_name = variant.ident.to_string();
        let label = format!("{rust_name}::{variant_name}");
        let c_type = if variant_needs_companion(&variant_fields(&variant.fields)) {
            Some(variant_type_name(repr, &variant_name))
        } else {
            payload_fields(&variant.fields)
                .first()
                .and_then(|(_, ty)| field_c_type(ty, &resolution, &mut reporting, &label))
        };

        let Some(c_type) = c_type else {
            continue;
        };
        body.push_str(&render_doc(&doc_lines(&variant.attrs), "  "));
        let _ = writeln!(body, "  {c_type} {variant_name};");
    }

    format!("typedef union {name} {{\n{body}}} {name};\n\n")
}

/// Reports two definitions that claim the same C name.
///
/// Nothing else catches this: two `#[lazyffi]` types with the same name in
/// different crates both map to the same repr, and two different `export =`
/// overrides can collide. C would reject the result, but only once a user tried to
/// compile it — so the build stops here instead.
fn duplicates(definitions: &[Definition]) -> Vec<Diagnostic> {
    let mut seen: BTreeMap<&str, &Definition> = BTreeMap::new();
    let mut diagnostics = Vec::new();

    for definition in definitions {
        if let Some(first) = seen.get(definition.name.as_str()) {
            diagnostics.push(Diagnostic {
                path: definition.origin.path.clone(),
                source: definition.origin.source.as_ref().clone(),
                line: definition.line,
                item: definition.item.clone(),
                label: format!("`{}` is already defined", definition.name),
                note: Some(format!(
                    "also exported as `{}` from {}:{}",
                    first.name,
                    first.origin.path.display(),
                    first.line
                )),
            });
        } else {
            seen.insert(definition.name.as_str(), definition);
        }
    }

    diagnostics
}

/// The state of one definition during the dependency walk.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Visit {
    /// Not reached yet.
    Pending,
    /// Being walked; reaching it again means a cycle.
    Active,
    /// Already emitted.
    Done,
}

/// Every C type name `text` mentions, ignoring `own` and anything not defined in
/// this header.
fn referenced(text: &str, names: &BTreeSet<String>, own: &str) -> BTreeSet<String> {
    text.split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|word| *word != own && names.contains(*word))
        .map(str::to_string)
        .collect()
}

/// Orders definitions so that every type is complete before it is used.
///
/// C cannot express a cycle of by-value types, so a cycle here means the sources
/// describe something the header cannot represent; it is reported rather than
/// emitted.
fn order(definitions: &[Definition]) -> Result<Vec<Definition>, Vec<Diagnostic>> {
    let names: BTreeSet<String> = definitions.iter().map(|item| item.name.clone()).collect();
    let index: BTreeMap<&str, usize> = definitions
        .iter()
        .enumerate()
        .map(|(position, item)| (item.name.as_str(), position))
        .collect();

    let dependencies = definitions
        .iter()
        .map(|item| {
            referenced(&item.text, &names, &item.name)
                .iter()
                .filter_map(|name| index.get(name.as_str()).copied())
                .collect::<Vec<usize>>()
        })
        .collect::<Vec<_>>();

    let mut state = vec![Visit::Pending; definitions.len()];
    let mut ordered = Vec::with_capacity(definitions.len());
    let mut diagnostics = Vec::new();

    for start in 0..definitions.len() {
        visit(
            start,
            &dependencies,
            definitions,
            &mut state,
            &mut ordered,
            &mut diagnostics,
        );
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(ordered
        .into_iter()
        .map(|position| definitions[position].clone())
        .collect())
}

/// Depth-first emission of one definition and its dependencies.
fn visit(
    node: usize,
    dependencies: &[Vec<usize>],
    definitions: &[Definition],
    state: &mut [Visit],
    ordered: &mut Vec<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match state[node] {
        Visit::Done => return,
        Visit::Active => {
            let definition = &definitions[node];
            push_diagnostic(
                diagnostics,
                &definition.origin,
                definition.line,
                &definition.item,
                format!(
                    "`{}` is defined in terms of itself; C cannot hold a by-value cycle",
                    definition.name
                ),
            );
            return;
        }
        Visit::Pending => {}
    }

    state[node] = Visit::Active;
    for &dependency in &dependencies[node] {
        visit(
            dependency,
            dependencies,
            definitions,
            state,
            ordered,
            diagnostics,
        );
    }
    state[node] = Visit::Done;
    ordered.push(node);
}

/// Renders the whole header, or reports everything it could not resolve.
fn render(exports: &Exports) -> Result<String, Vec<Diagnostic>> {
    let reprs = reprs(exports);
    let mut diagnostics = Vec::new();

    let mut definitions = Vec::new();
    build_definitions(exports, &reprs, &mut definitions, &mut diagnostics);
    diagnostics.extend(duplicates(&definitions));
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let definitions = order(&definitions)?;

    let mut out = String::from(BANNER);
    out.push_str(&preamble());

    for (item, origin) in &exports.constants {
        render_const(&mut out, item, origin, &reprs, &mut diagnostics);
    }
    for definition in &definitions {
        out.push_str(&definition.text);
    }
    for (item, origin) in &exports.functions {
        render_function(&mut out, item, origin, &reprs, &mut diagnostics);
    }
    for (item, origin) in &exports.impls {
        render_impl(&mut out, item, origin, &reprs, &mut diagnostics);
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    out.push_str(&footer());
    Ok(out)
}

/// Renders one exported constant.
fn render_const(
    out: &mut String,
    item: &ItemConst,
    origin: &Origin,
    reprs: &Reprs,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let rust_name = item.ident.to_string();
    let default = value_name(&rust_name);
    let export = export_override(&item.attrs).unwrap_or(default);

    if is_str_ref(&item.ty) {
        out.push_str(&render_doc(&doc_lines(&item.attrs), ""));
        let _ = writeln!(out, "char *{export}(void);\n");
        return;
    }

    let mut reporting = Reporting::new(origin, diagnostics);
    let Some(c_type) = field_c_type(
        &item.ty,
        &Resolution::plain(reprs, &origin.path),
        &mut reporting,
        &rust_name,
    ) else {
        return;
    };
    out.push_str(&render_doc(&doc_lines(&item.attrs), ""));
    let _ = writeln!(out, "extern const {c_type} {export};\n");
}

/// Renders the wrapper of one exported free function.
fn render_function(
    out: &mut String,
    item: &ItemFn,
    origin: &Origin,
    reprs: &Reprs,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let rust_name = item.sig.ident.to_string();
    let default = value_name(&rust_name);
    let export = export_override(&item.attrs).unwrap_or(default);
    let resolution = Resolution::plain(reprs, &origin.path);

    // The result mapping belongs to the function's own doc block: it is the only
    // place a caller is told what a payload is, and it is not something the Rust
    // source can say.
    let mut docs = doc_lines(&item.attrs);
    docs.extend(result_doc_lines(
        &item.sig,
        &resolution,
        &mut Reporting::new(origin, diagnostics),
        &rust_name,
    ));
    out.push_str(&render_doc(&docs, ""));

    render_signature(
        out,
        &item.sig,
        &export,
        None,
        &resolution,
        &mut Reporting::new(origin, diagnostics),
        &rust_name,
    );
}

/// Renders one wrapper per method of an exported `impl`.
fn render_impl(
    out: &mut String,
    item: &ItemImpl,
    origin: &Origin,
    reprs: &Reprs,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut reporting = Reporting::new(origin, diagnostics);

    if let Some((path, _)) = &item.trait_ {
        reporting.report(
            path.span().start().line,
            "impl",
            "`#[lazyffi]` only supports inherent `impl` blocks, not trait implementations"
                .to_string(),
        );
        return;
    }

    if let Some(export) = export_override(&item.attrs) {
        reporting.report(
            item.self_ty.span().start().line,
            "impl",
            format!(
                "`export = {export}` is not accepted on `impl`, since it would name every method \
                 the same; put `#[lazyffi(export = ...)]` on the method instead"
            ),
        );
        return;
    }

    let Some(self_name) = simple_type_name(&item.self_ty) else {
        reporting.report(
            item.self_ty.span().start().line,
            "impl",
            "`#[lazyffi]` needs a plain path as the `impl` target".to_string(),
        );
        return;
    };

    let Some(repr) = repr_of(&item.self_ty, &Resolution::plain(reprs, &origin.path))
        .and_then(|repr| c_type(&repr, reprs))
    else {
        reporting.report(
            item.self_ty.span().start().line,
            &self_name,
            format!("`{}` has no repr-C sibling", compact(&item.self_ty)),
        );
        return;
    };

    // Inside the `impl`, `Self` names the target type.
    let resolution = Resolution {
        reprs,
        origin: &origin.path,
        self_repr: Some(&repr),
    };

    for impl_item in &item.items {
        let ImplItem::Fn(method) = impl_item else {
            continue;
        };
        let rust_name = method.sig.ident.to_string();
        let default = method_name(&self_name, &rust_name);
        let export = export_override(&method.attrs).unwrap_or(default);
        let label = format!("{self_name}::{rust_name}");

        let mut receiver = None;
        if let Some(receiver_item) = method.sig.receiver() {
            match &receiver_item.kind {
                // A receiver borrows the repr itself, `const` when the method only
                // reads through it.
                ReceiverKind::Reference(_, _, mutability) => {
                    let constness = if mutability.is_none() { "const " } else { "" };
                    receiver = Some(format!("{constness}{repr} *"));
                }
                // A by-value receiver is handed over the way a value is: as a pointer
                // for an opaque type, by value otherwise.
                _ => receiver = Some(reprs.shape(&repr)),
            }
        }

        let mut docs = doc_lines(&method.attrs);
        docs.extend(result_doc_lines(
            &method.sig,
            &resolution,
            &mut reporting,
            &label,
        ));
        out.push_str(&render_doc(&docs, ""));
        render_signature(
            out,
            &method.sig,
            &export,
            receiver.as_deref(),
            &resolution,
            &mut reporting,
            &label,
        );
    }
}

/// Whether `#[lazyffi]` can export a signature at all.
///
/// The macro rejects these shapes too; checking them here as well means the build
/// stops with this message first, instead of after the wrapper has been rendered
/// into a plausible-looking but wrong declaration.
fn signature_is_exportable(sig: &Signature, reporting: &mut Reporting<'_>, item: &str) -> bool {
    if sig.asyncness.is_some() {
        reporting.report(
            sig.span().start().line,
            item,
            "`#[lazyffi]` does not support `async fn`".to_string(),
        );
        return false;
    }

    if !sig.generics.params.is_empty() {
        reporting.report(
            sig.generics.span().start().line,
            item,
            "`#[lazyffi]` does not support generic functions".to_string(),
        );
        return false;
    }

    if !matches!(sig.safety, syn::Safety::Default) {
        reporting.report(
            sig.span().start().line,
            item,
            "`#[lazyffi]` does not support `unsafe fn` or `safe fn`".to_string(),
        );
        return false;
    }

    true
}

/// The name of an `impl` target, when it is a plain path.
fn simple_type_name(ty: &Type) -> Option<String> {
    if let Type::Path(type_path) = ty
        && type_path.qself.is_none()
        && let Some(segment) = type_path.path.segments.last()
        && segment.arguments.is_none()
    {
        return Some(segment.ident.to_string());
    }

    None
}

/// The `Ok` and `Err` types of a `Result<T, E>` return, when the signature has one.
///
/// Matched on the last path segment, the way every other rule here is: an alias for
/// `Result` does not resolve, and saying so beats guessing.
fn result_return(sig: &Signature) -> Option<(&Type, &Type)> {
    let ReturnType::Type(_, ty) = &sig.output else {
        return None;
    };

    let Type::Path(type_path) = &**ty else {
        return None;
    };

    if type_path.qself.is_some() {
        return None;
    }

    let segment = type_path.path.segments.last()?;
    if segment.ident != "Result" {
        return None;
    }

    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };

    let types: Vec<&Type> = arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .collect();

    match types.as_slice() {
        [ok, error] => Some((ok, error)),
        _ => None,
    }
}

/// The doc lines a fallible return contributes: what each side of the tag holds, and
/// which release frees it.
///
/// They go into the function's own doc block because the payload is a `void *` — one C
/// layout cannot name two types — so the header is the only place that can tell a
/// caller what to cast it to.
fn result_doc_lines(
    sig: &Signature,
    resolution: &Resolution<'_>,
    reporting: &mut Reporting<'_>,
    item: &str,
) -> Vec<String> {
    let Some((ok, error)) = result_return(sig) else {
        return Vec::new();
    };

    let mut lines = vec![String::new(), "Returns a result:".to_string()];

    for (side, ty) in [(RESULT_OK_VARIANT, ok), (RESULT_ERR_VARIANT, error)] {
        if let Some(text) = payload_mapping(ty, resolution, reporting, item) {
            lines.push(format!("- `{side}`: {text}"));
        }
    }

    lines
}

/// One side of a result, in C's terms: what the caller casts the payload to, and what
/// releases it.
fn payload_mapping(
    ty: &Type,
    resolution: &Resolution<'_>,
    reporting: &mut Reporting<'_>,
    item: &str,
) -> Option<String> {
    // The `Ok` of a `Result<(), E>` carries nothing, and crosses as a null payload.
    if is_unit(ty) {
        return Some("nothing; the payload is null".to_string());
    }

    let Some(repr) = repr_of(ty, resolution) else {
        reporting.report(
            ty.span().start().line,
            item,
            format!(
                "`{}` has no repr-C sibling, so it cannot be a result payload",
                compact(ty)
            ),
        );
        return None;
    };

    // A string is the one payload that already crosses as a pointer of its own.
    if repr == STRING_REPR {
        return Some(format!("`char *`, owned; release it with `{FREE_STRING}`"));
    }

    // Everything else has a value repr, so it is boxed: a payload is a pointer.
    if SCALAR_NAMES.contains(&repr.as_str()) {
        reporting.report(
            ty.span().start().line,
            item,
            "a scalar cannot be a result payload: it has no pointer of its own, so C \
             would have nothing to cast and nothing to release"
                .to_string(),
        );
        return None;
    }

    let spelling = c_type(&repr, resolution.reprs)?;
    let release = resolution.reprs.releases.get(&repr)?;

    Some(format!(
        "`{spelling} *`, owned; release it with `{release}`"
    ))
}

/// Whether `ty` is the unit type, which a result uses for the side carrying nothing.
fn is_unit(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty())
}

/// Renders one C declaration from a Rust signature.
///
/// `receiver` is the already-rendered C type of a method receiver, if any: the
/// receiver has no C spelling of its own, since Rust writes it as `Self`.
fn render_signature(
    out: &mut String,
    sig: &Signature,
    export: &str,
    receiver: Option<&str>,
    resolution: &Resolution<'_>,
    reporting: &mut Reporting<'_>,
    item: &str,
) {
    if !signature_is_exportable(sig, reporting, item) {
        return;
    }

    let mut params = Vec::new();
    if let Some(receiver) = receiver {
        // `self` cannot collide with a Rust parameter name, which is why the
        // receiver is spelled this way in C.
        params.push(format!("{receiver} self"));
    }

    for input in &sig.inputs {
        let FnArg::Typed(pat_type) = input else {
            continue;
        };
        let Pat::Ident(pat_ident) = &*pat_type.pat else {
            reporting.report(
                pat_type.pat.span().start().line,
                item,
                "`#[lazyffi]` requires plain identifier parameters".to_string(),
            );
            continue;
        };

        let c_type = if read_only_string(&pat_type.ty, resolution) {
            Some(READ_ONLY_STRING.to_string())
        } else {
            match &*pat_type.ty {
                Type::Reference(reference) => {
                    // Only a `&mut` reaches this arm: `read_only_string` above has already
                    // claimed every shared reference to a string, since one crosses as a C
                    // string. A pointer to one is not a value either side can build.
                    if repr_of(&reference.elem, resolution).is_some_and(|repr| repr == STRING_REPR)
                    {
                        reporting.report(
                            pat_type.ty.span().start().line,
                            item,
                            "`&mut` of a string or a path is not supported: one crosses as a C \
                             string, and a pointer to one is not a value this side can build"
                                .to_string(),
                        );
                        continue;
                    }

                    // A pointer to the repr itself, `const` when Rust only reads
                    // through it — which is also what lets a C++ caller pass what it
                    // holds as `const`.
                    let constness = if reference.mutability.is_none() {
                        "const "
                    } else {
                        ""
                    };

                    pointee_c_type(&reference.elem, resolution, reporting, item)
                        .map(|inner| format!("{constness}{inner} *"))
                }

                other => field_c_type(other, resolution, reporting, item),
            }
        };

        if let Some(c_type) = c_type {
            params.push(format!("{c_type} {}", pat_ident.ident));
        }
    }

    let returns = match &sig.output {
        ReturnType::Default => "void".to_string(),
        ReturnType::Type(_, ty) => {
            // A fallible return crosses as the one fixed result type. The doc lines
            // written above are where its payloads were named, and where they were
            // checked for crossing at all.
            if result_return(sig).is_some() {
                RESULT_REPR.to_string()
            } else {
                let Some(c_type) = field_c_type(ty, resolution, reporting, item) else {
                    return;
                };
                c_type
            }
        }
    };

    if params.is_empty() {
        let _ = writeln!(out, "{returns} {export}(void);\n");
    } else {
        let _ = writeln!(out, "{returns} {export}({});\n", params.join(", "));
    }
}

/// Byte offset at which a 1-based `line` starts in `source`.
fn line_offset(source: &str, line: usize) -> Range<usize> {
    let mut offset = 0;
    for (index, text) in source.split_inclusive('\n').enumerate() {
        if index + 1 == line {
            let length = text.trim_end_matches('\n').len();
            return offset..offset + length;
        }
        offset += text.len();
    }

    0..0
}

/// Renders [`Diagnostic`]s with `annotate-snippets`.
fn render_diagnostics(diagnostics: &[Diagnostic]) -> String {
    let renderer = Renderer::styled().decor_style(DecorStyle::Unicode);
    let mut out = String::new();

    for diagnostic in diagnostics {
        let mut group = Level::ERROR
            .primary_title(format!("`{}` cannot be exported", diagnostic.item))
            .element(
                Snippet::source(&diagnostic.source)
                    .line_start(1)
                    .path(diagnostic.path.display().to_string())
                    .annotation(
                        AnnotationKind::Primary
                            .span(line_offset(&diagnostic.source, diagnostic.line))
                            .label(&diagnostic.label),
                    ),
            );

        if let Some(note) = &diagnostic.note {
            group = group.element(Level::NOTE.message(note.clone()));
        }

        out.push_str(&renderer.render(&[group]));
        // The renderer does not terminate its output, and the build script
        // forwards the message one line at a time.
        out.push('\n');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{Config, generate};
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Renders the header for a source tree holding exactly `source`.
    ///
    /// The generator's input is a directory, so a test has to build one. What it
    /// asserts on is the header, which is the artifact the build script hands out and
    /// the only thing a C caller ever sees — the Rust side is not what can be wrong.
    fn header_for(source: &str) -> String {
        header_for_each(&[("lib.rs", source)])
    }

    /// Renders the header for a source tree holding each of `sources`, one file each.
    fn header_for_each(sources: &[(&str, &str)]) -> String {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let root = std::env::temp_dir().join(format!(
            "rorolala-bindgen-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);

        let sources_dir = root.join("src");
        fs::create_dir_all(&sources_dir).unwrap();
        for (name, source) in sources {
            fs::write(sources_dir.join(name), source).unwrap();
        }

        let output_dir = root.join("out");
        let config = Config {
            source_roots: std::slice::from_ref(&sources_dir),
            output_dir: &output_dir,
        };

        fs::read_to_string(generate(&config).unwrap()).unwrap()
    }

    #[test]
    fn two_modules_may_name_a_type_the_same() {
        // The shape of this workspace: `vault` and `workspace` each have a `Config`,
        // and each `export`s it under a name of its own. A definition has to be
        // rendered from its own export rather than from a table keyed by a name both
        // of them have.
        let header = header_for_each(&[
            (
                "vault.rs",
                "/// A vault.\n#[lazyffi(export = VaultConfig)]\npub struct Config {}\n",
            ),
            (
                "workspace.rs",
                "/// A workspace.\n#[lazyffi(export = WorkspaceConfig)]\npub struct Config {}\n",
            ),
        ]);

        assert!(
            header.contains("typedef struct VaultConfig VaultConfig;"),
            "{header}"
        );
        assert!(
            header.contains("typedef struct WorkspaceConfig WorkspaceConfig;"),
            "{header}"
        );

        // The releases are named after the name C knows each type by, so the two do
        // not become one symbol at link time.
        assert!(
            header.contains("void free_vault_config(VaultConfig *value);"),
            "{header}"
        );
        assert!(
            header.contains("void free_workspace_config(WorkspaceConfig *value);"),
            "{header}"
        );
    }

    /// A counter with a read-only method, a mutating method and a free function that
    /// borrows one, which is every reference shape the generator has to spell.
    const REFERENCES: &str = "
/// A counter.
#[lazyffi]
pub struct Counter {}

#[lazyffi]
impl Counter {
    /// Reads the count.
    pub const fn value(&self) -> i64 { 0 }

    /// Advances the count.
    pub const fn advance(&mut self) {}
}

/// Adds one.
#[lazyffi]
pub const fn total(counter: &Counter) -> i64 { 0 }
";

    #[test]
    fn a_shared_reference_is_a_const_pointer() {
        let header = header_for(REFERENCES);

        // Rust only reads through these, so C is told so — which is also what lets a
        // C++ caller hand over something it holds as `const`.
        assert!(
            header.contains("int64_t ffi_total(const FFICounter * counter);"),
            "{header}"
        );
        assert!(
            header.contains("int64_t ffi_counter_value(const FFICounter * self);"),
            "{header}"
        );
    }

    #[test]
    fn a_mutable_reference_is_a_plain_pointer() {
        let header = header_for(REFERENCES);

        // The value is written back through it, so it is not `const`.
        assert!(
            header.contains("void ffi_counter_advance(FFICounter * self);"),
            "{header}"
        );
    }

    #[test]
    fn a_shared_reference_to_a_string_is_still_a_const_char_pointer() {
        let header = header_for(
            "
/// Greets.
#[lazyffi]
pub const fn greet(name: &str) {}

/// Renames.
#[lazyffi]
pub const fn rename(to: &std::path::Path) {}
",
        );

        // A string is built from the C string and borrowed back, so it keeps the
        // spelling it had before shared references had one of their own.
        assert!(
            header.contains("void ffi_greet(const char * name);"),
            "{header}"
        );
        assert!(
            header.contains("void ffi_rename(const char * to);"),
            "{header}"
        );
    }

    /// A counter, a reason it might refuse, and one export of each fallible shape:
    /// one returning a value, one returning nothing at all.
    const FALLIBLE: &str = "
/// Why a counter refused.
#[lazyffi]
pub enum Refusal {
    /// It is not allowed.
    Denied,
}

/// A counter.
#[lazyffi]
pub struct Counter {}

/// Reads a counter.
#[lazyffi]
pub fn read(counter: &Counter) -> Result<String, Refusal> { String::new() }

/// Starts a counter.
#[lazyffi]
pub fn open() -> Result<(), Refusal> { Ok(()) }
";

    #[test]
    fn the_result_type_is_declared_whether_or_not_anything_is_fallible() {
        // It is part of the surface rather than something generated per item, so a
        // translation unit that never calls a fallible export still has it.
        let header = header_for(
            "
/// Adds one.
#[lazyffi]
pub const fn total(value: i32) -> i32 { value }
",
        );

        assert!(
            header.contains("typedef struct RorolalaResult {"),
            "{header}"
        );
        assert!(header.contains("RorolalaResult_Ok = 0,"), "{header}");
        assert!(header.contains("RorolalaResult_Err = 1,"), "{header}");
    }

    #[test]
    fn a_fallible_return_comes_back_as_the_one_result_type() {
        let header = header_for(FALLIBLE);

        assert!(
            header.contains("RorolalaResult ffi_read(const FFICounter * counter);"),
            "{header}"
        );
        assert!(
            header.contains("RorolalaResult ffi_open(void);"),
            "{header}"
        );
    }

    #[test]
    fn a_fallible_return_says_what_each_payload_is_and_what_frees_it() {
        let header = header_for(FALLIBLE);

        // The payload is a `void *`, so the header is the only place a caller can
        // learn what to cast it to — and with what to release it.
        assert!(
            header.contains("- `Ok`: `char *`, owned; release it with `free_string`"),
            "{header}"
        );
        assert!(
            header.contains("- `Err`: `FFIRefusal *`, owned; release it with `free_refusal`"),
            "{header}"
        );
        // The `Ok` of a `Result<(), E>` has nothing to carry.
        assert!(
            header.contains("- `Ok`: nothing; the payload is null"),
            "{header}"
        );
    }
}
