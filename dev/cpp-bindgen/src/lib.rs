//! Generates the C++ headers for Rorolala's C ABI, from the C header `rororala-dev-bindgen` writes.
//!
//! The input is the header, not the Rust sources: what a C++ caller may name is exactly what the
//! header declares, so reading it is what keeps the two surfaces the same one. The output is the
//! mechanical half of `rola_plus_plus`: aliases into `rola::raw`, the release policies and
//! `Owned<T>`, the `RorolalaResult` unwrapping, the error tag mapping, the ABI assertions and the
//! path constants. The object model above it is hand-written.
//!
//! The generator compiles every file it wrote before leaving it in place. A generated header that
//! does not compile is worse than no header, so a failure deletes the output and reports the
//! compiler's own words.

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
use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// The files the generator writes, in the order the umbrella includes them.
const FILES: [&str; 6] = [
    "raw.hpp",
    "release.hpp",
    "outcome.hpp",
    "error_kind.hpp",
    "abi.hpp",
    "constants.hpp",
];

/// What one generation run needs.
#[derive(Debug)]
pub struct Config<'paths> {
    /// The C header to read.
    pub header: &'paths Path,
    /// The directory the generated headers are written to.
    pub gen_dir: &'paths Path,
    /// The ABI major the generated headers assert against; the header's own when absent.
    pub expected_abi: Option<u32>,
    /// The umbrella header to write, when one was asked for.
    pub umbrella: Option<&'paths Path>,
}

/// How much of the surface one run found, so a caller can print or test the counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    /// Every function declaration, which becomes one alias in `rola::raw`.
    pub functions: usize,
    /// Every type with a `free_*`, which becomes one `Release<T>` specialisation.
    pub handles: usize,
    /// Every function returning `RorolalaResult`, which becomes one outcome trait.
    pub outcomes: usize,
    /// Every error tag enum, which becomes one `kind_of` switch.
    pub error_families: usize,
    /// Every `char *` constant function, which becomes one wrapper.
    pub constants: usize,
}

/// What can go wrong while generating the headers.
#[derive(Debug)]
pub enum Error {
    /// A file could not be read or written.
    Io {
        /// The offending path.
        path: PathBuf,
        /// The underlying error.
        source: io::Error,
    },
    /// The header is not shaped the way this generator reads.
    Shape {
        /// What is wrong with it.
        reason: String,
    },
    /// No `clang++` was found to compile the generated headers.
    Tool {
        /// What was looked for.
        reason: String,
    },
    /// The compiler rejected a generated file, which was then removed.
    Compile {
        /// The file and the compiler's own diagnostic.
        output: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Shape { reason } | Self::Tool { reason } => formatter.write_str(reason),
            Self::Compile { output } => formatter.write_str(output),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Shape { .. } | Self::Tool { .. } | Self::Compile { .. } => None,
        }
    }
}

/// Generates the headers and returns how much of the surface they cover.
///
/// # Errors
///
/// Fails when the header cannot be read, when it declares a `RorolalaResult` without the two
/// `Ok` and `Err` lines, when a free function names no type, when no `clang++` is found, or when
/// the compiler rejects a generated file. The generated files are removed on the last of these.
pub fn generate(config: &Config<'_>) -> Result<Counts, Error> {
    let source = fs::read_to_string(config.header).map_err(|error| Error::Io {
        path: config.header.to_path_buf(),
        source: error,
    })?;

    let header = parse(&source)?;
    let surface = Surface::of(&header)?;
    let abi = config
        .expected_abi
        .map_or_else(|| header.abi_major.clone(), |major| major.to_string());

    let files = surface.render(&abi);
    let umbrella = config
        .umbrella
        .map(|path| render_umbrella(path, config.gen_dir));
    let counts = surface.counts();

    fs::create_dir_all(config.gen_dir).map_err(|error| Error::Io {
        path: config.gen_dir.to_path_buf(),
        source: error,
    })?;

    let mut written = Vec::new();
    for (name, text) in &files {
        let path = config.gen_dir.join(name);
        fs::write(&path, text).map_err(|error| Error::Io {
            path: path.clone(),
            source: error,
        })?;
        written.push(path);
    }

    if let (Some(umbrella), Some(text)) = (config.umbrella, &umbrella) {
        if let Some(parent) = umbrella
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| Error::Io {
                path: parent.to_path_buf(),
                source: error,
            })?;
        }

        fs::write(umbrella, text).map_err(|error| Error::Io {
            path: umbrella.to_path_buf(),
            source: error,
        })?;
        written.push(umbrella.to_path_buf());
    }

    match compile(config) {
        Ok(()) => Ok(counts),
        Err(error) => {
            for path in written {
                let _ = fs::remove_file(path);
            }
            Err(error)
        }
    }
}

/// Compiles every header just written, and the umbrella, so nothing unbuilt is kept.
fn compile(config: &Config<'_>) -> Result<(), Error> {
    let compiler = find_compiler().ok_or_else(|| Error::Tool {
        reason: "no `clang++` found, looked at `C:\\Program Files\\LLVM\\bin` and `PATH`"
            .to_owned(),
    })?;

    let header_dir = config.header.parent().unwrap_or_else(|| Path::new("."));
    let check = config.gen_dir.join("__cpp_bindgen_check.cpp");

    for name in FILES {
        let text = format!("#include \"{name}\"\n");
        fs::write(&check, text).map_err(|error| Error::Io {
            path: check.clone(),
            source: error,
        })?;

        run(compiler.as_path(), config, header_dir, &check).map_err(|output| {
            let _ = fs::remove_file(&check);
            Error::Compile {
                output: format!("{name}: {output}"),
            }
        })?;
    }

    if let Some(umbrella) = config.umbrella {
        let include = absolute(umbrella).to_string_lossy().replace('\\', "/");
        let text = format!("#include \"{include}\"\n");
        fs::write(&check, text).map_err(|error| Error::Io {
            path: check.clone(),
            source: error,
        })?;

        run(compiler.as_path(), config, header_dir, &check).map_err(|output| {
            let _ = fs::remove_file(&check);
            Error::Compile {
                output: format!("umbrella: {output}"),
            }
        })?;
    }

    let _ = fs::remove_file(&check);

    Ok(())
}

/// Runs the compiler over one check file and returns its diagnostic on failure.
fn run(
    compiler: &Path,
    config: &Config<'_>,
    header_dir: &Path,
    check: &Path,
) -> Result<(), String> {
    let output = Command::new(compiler)
        .arg("-std=c++23")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg("-fsyntax-only")
        .arg("-I")
        .arg(config.gen_dir)
        .arg("-I")
        .arg(header_dir)
        .arg(check)
        .output()
        .map_err(|error| format!("could not run {}: {error}", compiler.display()))?;

    if output.status.success() {
        return Ok(());
    }

    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    text.push_str(&String::from_utf8_lossy(&output.stderr));

    Err(text.trim().to_owned())
}

/// The compiler the generated headers are checked with.
fn find_compiler() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from(r"C:\Program Files\LLVM\bin\clang++.exe"),
        PathBuf::from("clang++"),
    ];

    for candidate in candidates {
        if candidate.components().count() > 1 && !candidate.exists() {
            continue;
        }

        if Command::new(&candidate)
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
        {
            return Some(candidate);
        }
    }

    None
}

/// The parts of the header the generator reads.
#[derive(Debug)]
struct Header {
    /// The ABI major, as the header spells it.
    abi_major: String,
    /// Every function declaration, in header order.
    functions: Vec<Function>,
    /// Every enum the header defines.
    enums: Vec<Enum>,
}

/// One function declaration.
#[derive(Debug)]
struct Function {
    /// Its C name.
    name: String,
    /// Its C return type, as written.
    ret: String,
    /// Its parameter list, as written.
    params: String,
    /// The lines of the doc comment above it.
    doc: Vec<String>,
}

/// One enum the header defines.
#[derive(Debug)]
struct Enum {
    /// Its C name.
    name: String,
    /// Its enumerators, in declaration order.
    variants: Vec<String>,
}

/// Reads the header, in the one style the generator writes it in.
fn parse(source: &str) -> Result<Header, Error> {
    let mut functions = Vec::new();
    let mut enums: Vec<Enum> = Vec::new();
    let mut abi_major = None;

    let mut doc: Vec<String> = Vec::new();
    let mut in_banner = false;
    let mut in_doc = false;
    let mut block: Option<Block> = None;

    for raw in source.lines() {
        if in_banner {
            if raw.contains("*/") {
                in_banner = false;
            }

            continue;
        }

        if in_doc {
            let (text, closed) = doc_tail(raw);
            if let Some(text) = text {
                doc.push(text);
            }

            if closed {
                in_doc = false;
            }

            continue;
        }

        let line = raw.trim();

        if let Some(open) = block.as_mut() {
            match open {
                Block::Enum { variants, .. } => {
                    if line.starts_with('}') {
                        if let Some(Block::Enum { name, variants }) = block.take() {
                            enums.push(Enum { name, variants });
                        }
                    } else if let Some(variant) = enum_variant(line) {
                        variants.push(variant);
                    }
                }
                Block::Other => {
                    if line.starts_with('}') {
                        block = None;
                    }
                }
            }

            continue;
        }

        if line.starts_with("/*!") {
            if !line.contains("*/") {
                in_banner = true;
            }

            continue;
        }

        if line.starts_with("/*") {
            let (text, closed) = doc_head(line);
            if let Some(text) = text {
                doc.push(text);
            }

            if !closed {
                in_doc = true;
            }

            continue;
        }

        if let Some(major) = abi_define(line) {
            abi_major = Some(major);

            continue;
        }

        if line.is_empty() || line.starts_with('#') || line == "}" {
            continue;
        }

        if let Some(name) = enum_open(line) {
            block = Some(Block::Enum {
                name,
                variants: Vec::new(),
            });
            doc.clear();

            continue;
        }

        if line.starts_with("typedef struct ") || line.starts_with("typedef union ") {
            if line.contains('{') {
                block = Some(Block::Other);
            }

            doc.clear();

            continue;
        }

        if line.starts_with("typedef ") || line.starts_with("ROROLALA_ABI_STATIC_ASSERT") {
            doc.clear();

            continue;
        }

        if let Some((name, ret, params)) = function(line) {
            functions.push(Function {
                name,
                ret,
                params,
                doc: take_doc(&mut doc),
            });
        } else {
            doc.clear();
        }
    }

    let abi_major =
        abi_major.ok_or_else(|| shape("the header states no `ROROLALA_ABI_VERSION_MAJOR`"))?;

    Ok(Header {
        abi_major,
        functions,
        enums,
    })
}

/// What the parser is inside of.
#[derive(Debug)]
enum Block {
    /// An enum, whose enumerators are being collected.
    Enum {
        /// Its C name.
        name: String,
        /// The enumerators read so far.
        variants: Vec<String>,
    },
    /// A struct or union body, whose contents are stepped over.
    Other,
}

/// The name in a `typedef enum Name {` line.
fn enum_open(line: &str) -> Option<String> {
    let rest = line.strip_prefix("typedef enum ")?.trim();
    if !line.contains('{') {
        return None;
    }

    let name = rest
        .split(|c: char| c == '{' || c.is_whitespace())
        .next()?
        .trim();

    (!name.is_empty()).then(|| name.to_owned())
}

/// One enumerator of an enum body, or nothing when the line is not one.
fn enum_variant(line: &str) -> Option<String> {
    let head = line.split(['=', ',']).next()?.trim();

    if head.is_empty()
        || !head.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        || head.starts_with(|c: char| c.is_ascii_digit())
    {
        return None;
    }

    Some(head.to_owned())
}

/// The ABI major from a `#define ROROLALA_ABI_VERSION_MAJOR n` line.
fn abi_define(line: &str) -> Option<String> {
    let rest = line.strip_prefix("#define ROROLALA_ABI_VERSION_MAJOR")?;

    Some(rest.trim().to_owned())
}

/// One function declaration as name, return type and parameter list.
fn function(line: &str) -> Option<(String, String, String)> {
    let body = line.strip_suffix(");")?;
    let open = body.find('(')?;
    let head = body.get(..open)?.trim();
    let name_at = head
        .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .map_or(0, |at| at + 1);

    let (ret, name) = head.split_at(name_at);
    let ret = ret.trim();
    let name = name.trim();

    if ret.is_empty() || name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }

    let params = body.get(open + 1..)?.trim().to_owned();

    Some((name.to_owned(), ret.to_owned(), params))
}

/// The text of an opening doc line, and whether it closes on the same line.
fn doc_head(line: &str) -> (Option<String>, bool) {
    let inner = line.strip_prefix("/*").unwrap_or(line);
    let inner = inner.strip_prefix('*').unwrap_or(inner);
    let closed = inner.contains("*/");
    let body = inner.split("*/").next().unwrap_or_default().trim();

    (
        Some(body.to_owned()).filter(|text| !text.is_empty()),
        closed,
    )
}

/// The text of a continuing doc line, and whether it closes.
fn doc_tail(line: &str) -> (Option<String>, bool) {
    let text = line.trim();
    let closed = text.contains("*/");
    let body = text.split("*/").next().unwrap_or_default();
    let body = body.trim();
    let body = body.strip_prefix('*').unwrap_or(body).trim();

    (
        Some(body.to_owned()).filter(|text| !text.is_empty()),
        closed,
    )
}

/// Takes the doc accumulated so far, without its empty edges.
fn take_doc(doc: &mut Vec<String>) -> Vec<String> {
    while doc.last().is_some_and(String::is_empty) {
        doc.pop();
    }

    std::mem::take(doc)
}

/// A reason the header is not read, as an error.
fn shape(reason: &str) -> Error {
    Error::Shape {
        reason: reason.to_owned(),
    }
}

/// The C++ surface derived from the header, ready to render.
#[derive(Debug)]
struct Surface {
    /// Every function, with the name it is aliased by.
    raw: Vec<(String, String)>,
    /// Every type with a `free_*`, with that function.
    handles: BTreeMap<String, String>,
    /// Every outcome, keyed by function name.
    outcomes: Vec<Outcome>,
    /// Every error family, with its tag type and C++ members.
    errors: Vec<Family>,
    /// Every constant function, with the C++ name and whether it is a path.
    constants: Vec<Constant>,
}

/// One function's pair of payload types.
#[derive(Debug)]
struct Outcome {
    /// The function it unwraps.
    function: String,
    /// What `Ok` names.
    ok: Payload,
    /// What `Err` names.
    err: Payload,
}

/// One side of a `Returns a result:` pair.
#[derive(Debug)]
enum Payload {
    /// `nothing; the payload is null`.
    Nothing,
    /// `char *`, released with `free_string`.
    String,
    /// An owned handle, released with its own `free_*`.
    Handle(String),
}

/// One error tag enum and the enumeration it maps to.
#[derive(Debug)]
struct Family {
    /// The C tag type.
    tag: String,
    /// Its members, as `(C enumerator, C++ member)`.
    members: Vec<(String, String)>,
}

/// One path or name constant.
#[derive(Debug)]
struct Constant {
    /// The C function.
    function: String,
    /// The C++ wrapper's name.
    name: String,
    /// Whether it names a filesystem path rather than plain text.
    path: bool,
}

impl Surface {
    /// Derives every part of the surface, or fails on a shape it cannot read.
    fn of(header: &Header) -> Result<Self, Error> {
        let handles = handles(header);

        Ok(Self {
            raw: raw(header)?,
            outcomes: outcomes(header, &handles)?,
            errors: errors(header)?,
            constants: constants(header),
            handles,
        })
    }

    /// How much of each kind of declaration the surface holds.
    fn counts(&self) -> Counts {
        Counts {
            functions: self.raw.len(),
            handles: self.handles.len(),
            outcomes: self.outcomes.len(),
            error_families: self.errors.len(),
            constants: self.constants.len(),
        }
    }

    /// Renders the six generated files, in the order the umbrella includes them.
    fn render(&self, abi: &str) -> Vec<(String, String)> {
        vec![
            ("raw.hpp".to_owned(), self.render_raw()),
            ("release.hpp".to_owned(), self.render_release()),
            ("outcome.hpp".to_owned(), self.render_outcome()),
            ("error_kind.hpp".to_owned(), self.render_error_kind()),
            ("abi.hpp".to_owned(), render_abi(abi)),
            ("constants.hpp".to_owned(), self.render_constants()),
        ]
    }

    /// `raw.hpp`: every function introduced into `rola::raw`.
    fn render_raw(&self) -> String {
        let mut out = banner("raw.hpp");
        out.push_str("#pragma once\n\n#include \"rorolala.h\"\n\nnamespace rola::raw {\n\n");

        for (alias, function) in &self.raw {
            let _ = writeln!(out, "inline constexpr auto {alias} = ::{function};");
        }

        out.push_str("\n} // namespace rola::raw\n");
        out
    }

    /// `release.hpp`: one policy per freed type, and the owning aliases.
    fn render_release(&self) -> String {
        let mut out = banner("release.hpp");
        out.push_str(
            "#pragma once\n\n#include \"rorolala.h\"\n\n#include <memory>\n\nnamespace rola::detail {\n\n",
        );
        out.push_str("template <class T> struct Release;\n\n");

        for (ty, free) in &self.handles {
            let _ = write!(out, "template <> struct Release<{ty}> {{");
            let _ = writeln!(
                out,
                "\n    void operator()({ty} *handle) const noexcept {{ ::{free}(handle); }}\n}};\n"
            );
        }

        out.push_str(
            "struct StringRelease {\n    void operator()(char *text) const noexcept { ::free_string(text); }\n};\n\n",
        );
        out.push_str("} // namespace rola::detail\n\nnamespace rola {\n\n");
        out.push_str("template <class T> using Owned = std::unique_ptr<T, detail::Release<T>>;\n");
        out.push_str("using OwnedString = std::unique_ptr<char, detail::StringRelease>;\n\n");
        out.push_str("} // namespace rola\n");
        out
    }

    /// `outcome.hpp`: the `RorolalaResult` unwrapping and one trait per outcome.
    fn render_outcome(&self) -> String {
        let mut out = banner("outcome.hpp");
        out.push_str(
            "#pragma once\n\n#include \"release.hpp\"\n#include \"rorolala.h\"\n\n#include <expected>\n#include <utility>\n\nnamespace rola {\n\n",
        );
        out.push_str(
            "struct Nothing {};\n\nnamespace detail {\n\ntemplate <class Ok> struct PayloadOf {\n    static Ok wrap(void *payload) noexcept { return Ok{static_cast<typename Ok::pointer>(payload)}; }\n};\n\n",
        );
        out.push_str(
            "template <> struct PayloadOf<Nothing> {\n    static Nothing wrap(void *payload) noexcept {\n        if (payload != nullptr) {\n            StringRelease{}(static_cast<char *>(payload));\n        }\n        return {};\n    }\n};\n\n",
        );
        out.push_str(
            "template <> struct PayloadOf<OwnedString> {\n    static OwnedString wrap(void *payload) noexcept { return OwnedString{static_cast<char *>(payload)}; }\n};\n\n",
        );
        out.push_str("template <auto Fn> struct OutcomeType;\n\n");

        for outcome in &self.outcomes {
            let _ = writeln!(
                out,
                "template <> struct OutcomeType<&::{}> {{",
                outcome.function
            );
            let _ = writeln!(out, "    using Ok = {};", cpp_payload(&outcome.ok));
            let _ = writeln!(out, "    using Err = {};", cpp_payload(&outcome.err));
            out.push_str("};\n\n");
        }

        out.push_str(
            "template <auto Fn>\nauto take(RorolalaResult result)\n    -> std::expected<typename OutcomeType<Fn>::Ok, typename OutcomeType<Fn>::Err> {\n    using traits = OutcomeType<Fn>;\n    if (result.tag == RorolalaResult_Ok) {\n        return PayloadOf<typename traits::Ok>::wrap(result.payload);\n    }\n    return std::unexpected(PayloadOf<typename traits::Err>::wrap(result.payload));\n}\n\n",
        );
        out.push_str("} // namespace detail\n\nusing detail::take;\n\n} // namespace rola\n");
        out
    }

    /// `error_kind.hpp`: every error tag value in one enum, and one switch per family.
    ///
    /// The names live under `rola::raw` because `rola::ErrorKind` is the one unified kind a
    /// caller branches on; this enum only says which native family member a tag is.
    fn render_error_kind(&self) -> String {
        let mut out = banner("error_kind.hpp");
        out.push_str("#pragma once\n\n#include \"rorolala.h\"\n\nnamespace rola::raw {\n\nenum class ErrorKind {\n    unknown,\n");

        for family in &self.errors {
            for (_, member) in &family.members {
                let _ = writeln!(out, "    {member},");
            }
        }

        out.push_str("};\n\n");

        for family in &self.errors {
            let _ = writeln!(
                out,
                "inline ErrorKind kind_of({} tag) noexcept {{",
                family.tag
            );
            out.push_str("    switch (tag) {\n");

            for (value, member) in &family.members {
                let _ = writeln!(out, "    case {value}: return ErrorKind::{member};");
            }

            out.push_str("    }\n\n    return ErrorKind::unknown;\n}\n\n");
        }

        out.push_str("} // namespace rola::raw\n");
        out
    }

    /// `abi.hpp`: the version and size assertions.
    /// `constants.hpp`: every path and name constant wrapped as a value.
    fn render_constants(&self) -> String {
        let mut out = banner("constants.hpp");
        out.push_str(
            "#pragma once\n\n#include \"release.hpp\"\n#include \"rorolala.h\"\n\n#include <filesystem>\n#include <string>\n\nnamespace rola {\n\n",
        );

        for constant in &self.constants {
            if constant.path {
                let _ = writeln!(
                    out,
                    "inline std::filesystem::path {}() {{\n    return OwnedString{{::{}()}}.get();\n}}\n",
                    constant.name, constant.function
                );
            } else {
                let _ = writeln!(
                    out,
                    "inline std::string {}() {{\n    return std::string{{OwnedString{{::{}()}}.get()}};\n}}\n",
                    constant.name, constant.function
                );
            }
        }

        out.push_str("} // namespace rola\n");
        out
    }
}

/// `abi.hpp`: the version and size assertions.
fn render_abi(abi: &str) -> String {
    let mut out = banner("abi.hpp");
    out.push_str("#pragma once\n\n#include \"rorolala.h\"\n\n");
    out.push_str("// The ABI major this layer was generated for. It is overridable so a caller can pin it.\n");
    out.push_str("#ifndef ROLA_PLUS_PLUS_REQUIRED_ABI_MAJOR\n");
    let _ = writeln!(out, "#define ROLA_PLUS_PLUS_REQUIRED_ABI_MAJOR {abi}");
    out.push_str("#endif\n\n");
    out.push_str(
        "static_assert(ROROLALA_ABI_VERSION_MAJOR == ROLA_PLUS_PLUS_REQUIRED_ABI_MAJOR,\n              \"this rola++ was generated for another ABI major\");\n",
    );
    out.push_str(
        "static_assert(sizeof(RorolalaResult) == ROROLALA_ABI_SIZE_ROROLALARESULT,\n              \"RorolalaResult layout changed\");\n",
    );
    out
}

/// The umbrella, which includes the six generated files and nothing else yet.
fn render_umbrella(umbrella: &Path, gen_dir: &Path) -> String {
    let mut out = banner("rola_plus_plus.hpp");
    out.push_str("#pragma once\n\n");

    let from = umbrella.parent().unwrap_or_else(|| Path::new("."));
    for name in FILES {
        let include = relative_path(from, &gen_dir.join(name));
        let include = include.to_string_lossy().replace('\\', "/");
        let _ = writeln!(out, "#include \"{include}\"");
    }

    out
}

/// Every function, with the name it is aliased by, refusing a name collision.
fn raw(header: &Header) -> Result<Vec<(String, String)>, Error> {
    let mut aliases = BTreeSet::new();
    let mut raw = Vec::with_capacity(header.functions.len());

    for function in &header.functions {
        let alias = strip_prefix(&function.name);
        if !aliases.insert(alias.clone()) {
            return Err(shape(&format!(
                "`{}` and another declaration share the C++ name `{alias}`",
                function.name
            )));
        }

        raw.push((alias, function.name.clone()));
    }

    Ok(raw)
}

/// Every type with a `free_*`, paired with the function that releases it.
fn handles(header: &Header) -> BTreeMap<String, String> {
    let mut handles = BTreeMap::new();

    for function in &header.functions {
        if function.ret != "void" || !function.name.starts_with("free_") {
            continue;
        }

        if let Some(ty) = freed_type(&function.name, &function.params) {
            handles.insert(ty, function.name.clone());
        }
    }

    handles
}

/// Every function returning `RorolalaResult`, with the pair its doc names.
fn outcomes(header: &Header, handles: &BTreeMap<String, String>) -> Result<Vec<Outcome>, Error> {
    let mut outcomes = Vec::new();

    for function in &header.functions {
        if function.ret != "RorolalaResult" {
            continue;
        }

        let (ok, err) = result_types(&function.doc).ok_or_else(|| {
            shape(&format!(
                "`{}` returns `RorolalaResult` without the `Ok` and `Err` doc lines",
                function.name
            ))
        })?;

        for payload in [&ok, &err] {
            if let Payload::Handle(ty) = payload
                && !handles.contains_key(ty)
            {
                return Err(shape(&format!(
                    "`{}` names `{ty}`, which no `free_*` releases",
                    function.name
                )));
            }
        }

        if matches!(err, Payload::Nothing) {
            return Err(shape(&format!(
                "`{}` names `nothing` for `Err`, which cannot be empty",
                function.name
            )));
        }

        outcomes.push(Outcome {
            function: function.name.clone(),
            ok,
            err,
        });
    }

    Ok(outcomes)
}

/// Every error tag enum, with the members it maps into `rola::raw::ErrorKind`.
///
/// A family is an enum whose base name ends in `Error`. That keeps a reason or a kind
/// (`RolaConfigureErrorReason`, `RolaIoErrorKind`) out of the enumeration: they are
/// fields of an error, not a family a caller branches on.
fn errors(header: &Header) -> Result<Vec<Family>, Error> {
    let mut errors = Vec::new();
    let mut used = BTreeSet::new();

    for item in &header.enums {
        let base = item.name.trim_end_matches("Tag");
        if !base.starts_with("Rola") || !base.ends_with("Error") {
            continue;
        }

        let tag_prefix = item.name.trim_end_matches("Tag");
        let family = snake_case(
            item.name
                .strip_prefix("Rola")
                .unwrap_or(&item.name)
                .trim_end_matches("Tag")
                .trim_end_matches("Kind")
                .trim_end_matches("Reason"),
        );

        let mut members = Vec::new();
        for variant in &item.variants {
            let short = variant
                .strip_prefix(&format!("{tag_prefix}_"))
                .or_else(|| variant.strip_prefix(&format!("{}_", item.name)))
                .ok_or_else(|| {
                    shape(&format!(
                        "`{variant}` in `{}` does not carry the enum's prefix",
                        item.name
                    ))
                })?;
            let member = format!("{family}_{}", snake_case(short));

            if !used.insert(member.clone()) {
                return Err(shape(&format!(
                    "two error values both map to `raw::ErrorKind::{member}`"
                )));
            }

            members.push((variant.clone(), member));
        }

        errors.push(Family {
            tag: item.name.clone(),
            members,
        });
    }

    Ok(errors)
}

/// Every `char *` constant, as a wrapper that owns what it returns.
fn constants(header: &Header) -> Vec<Constant> {
    let mut constants = Vec::new();

    for function in &header.functions {
        if function.ret != "char *" || !function.name.starts_with("ROLA_") {
            continue;
        }

        let name = function
            .name
            .strip_prefix("ROLA_")
            .unwrap_or(&function.name)
            .to_ascii_lowercase();
        let path = ["_DIR", "_PATH", "_FILE"]
            .iter()
            .any(|suffix| function.name.ends_with(suffix));

        constants.push(Constant {
            function: function.name.clone(),
            name,
            path,
        });
    }

    constants
}

/// The C++ name of one side of an outcome.
fn cpp_payload(payload: &Payload) -> String {
    match payload {
        Payload::Nothing => "Nothing".to_owned(),
        Payload::String => "OwnedString".to_owned(),
        Payload::Handle(ty) => format!("Owned<{ty}>"),
    }
}

/// The type a `free_*` releases, or nothing when the function is not one.
fn freed_type(name: &str, params: &str) -> Option<String> {
    if name == "free_string" {
        return None;
    }

    let ty = params.split('*').next()?.trim();
    let ty = ty.strip_prefix("const ").unwrap_or(ty).trim();

    if ty.is_empty() || ty == "char" || ty == "void" {
        return None;
    }

    Some(ty.to_owned())
}

/// The pair of types under a `Returns a result:` heading.
fn result_types(doc: &[String]) -> Option<(Payload, Payload)> {
    let mut at = 0;

    while at < doc.len() {
        if doc.get(at)?.trim() != "Returns a result:" {
            at += 1;

            continue;
        }

        let mut ok = None;
        let mut err = None;
        let mut line = at + 1;

        while let Some(text) = doc.get(line) {
            let text = text.trim();
            let Some(rest) = text.strip_prefix("- ") else {
                break;
            };

            if let Some(value) = rest.strip_prefix("`Ok`:") {
                ok = result_side(value);
            } else if let Some(value) = rest.strip_prefix("`Err`:") {
                err = result_side(value);
            } else {
                break;
            }

            line += 1;
        }

        if ok.is_some() || err.is_some() {
            return Some((ok?, err?));
        }

        at += 1;
    }

    None
}

/// One side of a result, from the text after `Ok` or `Err`.
fn result_side(text: &str) -> Option<Payload> {
    let text = text.trim();

    if text.starts_with("nothing") {
        return Some(Payload::Nothing);
    }

    let inner = text.strip_prefix('`')?;
    let c_type = inner.split('`').next()?.split('*').next()?.trim();

    if c_type == "char" {
        Some(Payload::String)
    } else if c_type.is_empty() {
        None
    } else {
        Some(Payload::Handle(c_type.to_owned()))
    }
}

/// The C++ alias of a C function, without the `rola_` the namespace already says.
fn strip_prefix(name: &str) -> String {
    name.strip_prefix("rola_").unwrap_or(name).to_owned()
}

/// `PascalCase` to `snake_case`, keeping acronyms whole.
fn snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);

    for (at, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let previous = at.checked_sub(1).and_then(|before| chars.get(before));
            let next = chars.get(at + 1);
            let after_lower =
                previous.is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
            let acronym_end = previous.is_some_and(char::is_ascii_uppercase)
                && next.is_some_and(char::is_ascii_lowercase);

            if at > 0 && (after_lower || acronym_end) {
                out.push('_');
            }

            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }

    out
}

/// The banner every generated file opens with.
fn banner(name: &str) -> String {
    format!(
        "// <auto-generated />\n//\n// Generated from `rorolala.h` by `cpp-bindgen` {}. DO NOT EDIT.\n// This is `{name}`, one of the mechanical headers under `rola_plus_plus/gen`.\n\n",
        env!("CARGO_PKG_VERSION")
    )
}

/// A path made absolute against the working directory, for a relative include.
fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }

    std::env::current_dir().map_or_else(|_| path.to_path_buf(), |dir| dir.join(path))
}

/// The path of `target` as seen from `from`, falling back to its absolute form.
fn relative_path(from: &Path, target: &Path) -> PathBuf {
    let from = absolute(from);
    let target_absolute = absolute(target);
    let mut from_parts = from.components().peekable();
    let mut target_parts = target_absolute.components().peekable();

    while from_parts.peek().is_some() && from_parts.peek() == target_parts.peek() {
        from_parts.next();
        target_parts.next();
    }

    if from_parts
        .peek()
        .is_some_and(|part| matches!(part, Component::Prefix(_) | Component::RootDir))
    {
        return target_absolute;
    }

    let mut out = PathBuf::new();
    for part in from_parts {
        if matches!(part, Component::Normal(_)) {
            out.push("..");
        }
    }

    for part in target_parts {
        out.push(part.as_os_str());
    }

    if out.as_os_str().is_empty() {
        return PathBuf::from(".");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A header in the shape `rororala-dev-bindgen` writes.
    const SAMPLE: &str = "\
/*!
 * This file is automatically generated.
 */

#ifndef ROROLALA_FFI_H
#define ROROLALA_FFI_H

#define ROROLALA_ABI_VERSION_MAJOR 1

#ifdef __cplusplus
extern \"C\" {
#endif

void free_string(char *string);

/** Hash salt. */
char *ROLA_SALT_VRT(void);

typedef struct RolaThing RolaThing;

void free_rola_thing(RolaThing *value);

typedef struct RolaThingError RolaThingError;

void free_rola_thing_error(RolaThingError *value);

typedef enum RolaThingErrorTag {
  /** Nothing there. */
  RolaThingError_Missing = 0,
  /** Broken. */
  RolaThingError_Broken = 1,
} RolaThingErrorTag;

/**
 * Reads a thing.
 *
 * Returns a result:
 * - `Ok`: `RolaThing *`, owned; release it with `free_rola_thing`
 * - `Err`: `RolaThingError *`, owned; release it with `free_rola_thing_error`
 */
RorolalaResult rola_thing_read(const char *path);

/**
 * Clears a thing.
 *
 * Returns a result:
 * - `Ok`: nothing; the payload is null
 * - `Err`: `RolaThingError *`, owned; release it with `free_rola_thing_error`
 */
RorolalaResult rola_thing_clear(const char *path);

/**
 * Writes a thing.
 *
 * Returns a result:
 * - `Ok`: `char *`, owned; release it with `free_string`
 * - `Err`: `RolaThingError *`, owned; release it with `free_rola_thing_error`
 */
RorolalaResult rola_thing_write(const char *path);

#ifdef __cplusplus
}
#endif

#endif  /* ROROLALA_FFI_H */
";

    #[test]
    fn reads_the_result_doc_lines() {
        let items = parse(SAMPLE).unwrap_or_else(|error| panic!("{error}"));
        let surface = Surface::of(&items).unwrap_or_else(|error| panic!("{error}"));

        assert_eq!(surface.counts().functions, 7);
        assert_eq!(surface.counts().handles, 2);
        assert_eq!(surface.counts().outcomes, 3);
        assert_eq!(surface.counts().error_families, 1);
        assert_eq!(surface.counts().constants, 1);

        let read = &surface.outcomes[0];
        assert_eq!(read.function, "rola_thing_read");
        assert!(matches!(read.ok, Payload::Handle(ref ty) if ty == "RolaThing"));
        assert!(matches!(read.err, Payload::Handle(ref ty) if ty == "RolaThingError"));

        let clear = &surface.outcomes[1];
        assert!(matches!(clear.ok, Payload::Nothing));

        let write = &surface.outcomes[2];
        assert!(matches!(write.ok, Payload::String));
        assert!(matches!(write.err, Payload::Handle(ref ty) if ty == "RolaThingError"));
    }

    #[test]
    fn refuses_an_outcome_without_both_lines() {
        let broken = SAMPLE.replace(
            " * - `Err`: `RolaThingError *`, owned; release it with `free_rola_thing_error`\n",
            "",
        );
        let items = parse(&broken).unwrap_or_else(|error| panic!("{error}"));

        assert!(Surface::of(&items).is_err());
    }

    #[test]
    fn renders_aliases_and_release() {
        let items = parse(SAMPLE).unwrap_or_else(|error| panic!("{error}"));
        let surface = Surface::of(&items).unwrap_or_else(|error| panic!("{error}"));
        let raw = surface.render_raw();
        let release = surface.render_release();

        assert!(raw.contains("namespace rola::raw {"));
        assert!(raw.contains("inline constexpr auto thing_read = ::rola_thing_read;"));
        assert!(raw.contains("inline constexpr auto ROLA_SALT_VRT = ::ROLA_SALT_VRT;"));
        assert!(release.contains("template <> struct Release<RolaThing>"));
        assert!(release.contains("::free_rola_thing(handle);"));
        assert!(
            release.contains("using OwnedString = std::unique_ptr<char, detail::StringRelease>;")
        );
    }

    #[test]
    fn maps_error_tags_into_one_enum() {
        let items = parse(SAMPLE).unwrap_or_else(|error| panic!("{error}"));
        let surface = Surface::of(&items).unwrap_or_else(|error| panic!("{error}"));
        let error_kind = surface.render_error_kind();

        assert!(error_kind.contains("namespace rola::raw {"));
        assert!(error_kind.contains("enum class ErrorKind {"));
        assert!(error_kind.contains("thing_error_missing,"));
        assert!(
            error_kind
                .contains("case RolaThingError_Missing: return ErrorKind::thing_error_missing;")
        );
        assert!(error_kind.contains("inline ErrorKind kind_of(RolaThingErrorTag tag) noexcept"));
        assert!(error_kind.contains("} // namespace rola::raw"));
    }

    #[test]
    fn generates_and_compiles_the_real_header() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let header = manifest.join("../../build/lib/rorolala.h");
        let gen_dir = manifest.join("../../__cpp_bindgen_test");
        let umbrella = gen_dir.join("rola_plus_plus.hpp");

        let _ = fs::remove_dir_all(&gen_dir);

        let config = Config {
            header: &header,
            gen_dir: &gen_dir,
            expected_abi: None,
            umbrella: Some(&umbrella),
        };

        let counts = generate(&config).unwrap_or_else(|error| panic!("{error}"));

        assert!(counts.functions > 500, "{counts:?}");
        assert!(counts.handles > 100, "{counts:?}");
        assert_eq!(counts.outcomes, 109, "{counts:?}");
        assert!(counts.error_families > 10, "{counts:?}");
        assert_eq!(counts.constants, 38, "{counts:?}");

        assert!(umbrella.exists());

        let _ = fs::remove_dir_all(&gen_dir);
    }
}
