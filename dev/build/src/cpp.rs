//! The C++ hand-off: the include tree a `rola++` consumer compiles against, and the `CMake` package
//! that names it.
//!
//! What is laid out is what the hand-written layer holds plus what the generator writes, under one
//! include root, so a consumer adds one `-I` and the layer's own `<rola_plus_plus/...>` includes
//! resolve. The C header is copied beside them for the generated files, which include
//! `"rorolala.h"` by name.
//!
//! Every path below is relative to the include root and to the library directory, so the whole
//! hand-off can be moved and the `CMake` package still finds the files. That is why the templates
//! carry no absolute path.

use std::path::Path;

use crate::Error;
use crate::fs;
use crate::paths::Workspace;
use crate::platform::Platform;

/// The hand-written headers, from the workspace root.
const SOURCE_INCLUDE: &str = "modules/rola-plus-plus/include";

/// The umbrella the hand-written layer includes the generated files through. It is generated
/// rather than carried, because it names every generated file and the generator is what knows them.
const GENERATED_UMBRELLA: &str = "rola_plus_plus/gen/rola_plus_plus.gen.hpp";

/// The `CMake` templates, from the workspace root.
const CMAKE_TEMPLATES: &str = "modules/rola-plus-plus/cmake";

/// The three files a `CMake` package is made of. The version file is separate because `find_package`
/// reads it before the configuration file, so a caller asking for a version is answered without
/// entering the package.
const CMAKE_FILES: [&str; 3] = [
    "rola_plus_plusConfig.cmake",
    "rola_plus_plusConfigVersion.cmake",
    "rola_plus_plusTargets.cmake",
];

/// Lays the include tree a C++ consumer compiles against out under `include_root`.
///
/// The hand-written headers are copied first, then the C header, then the generated layer: the
/// generator's own compile step reads the umbrella, which includes both layers, so a generated
/// header that does not compile fails here rather than in a consumer's build, and nothing half
/// written is left behind.
///
/// # Errors
///
/// Reports [`Error::Io`] naming the path that could not be copied or written, and the generator's
/// own failure when it refuses the header or when the compiler rejects a generated file.
pub fn lay_out_includes(
    workspace: &Workspace,
    include_root: &Path,
    header: &Path,
) -> Result<(), Error> {
    let source = workspace.root().join(SOURCE_INCLUDE);

    // The tree is replaced rather than merged: a header removed from the module would otherwise
    // stay in the layout and be compiled by every consumer of it.
    fs::remove(include_root)?;
    fs::copy_directory(&source, include_root)?;
    fs::copy_file(header, &include_root.join("rorolala.h"))?;

    let gen_dir = include_root.join("rola_plus_plus").join("gen");
    let umbrella = include_root.join(GENERATED_UMBRELLA);

    // The generator reads the header from beside the include root, not from the profile directory
    // it came from: the files it writes include `"rorolala.h"`, and its own compile step searches
    // the directory the header it was handed sits in.
    let local = include_root.join("rorolala.h");
    let config = rororala_dev_cpp_bindgen::Config {
        header: &local,
        gen_dir: &gen_dir,
        expected_abi: None,
        umbrella: Some(&umbrella),
    };

    rororala_dev_cpp_bindgen::generate(&config)
        .map(|_| ())
        .map_err(|error| Error::Io {
            doing: "generating the C++ headers",
            detail: error.to_string(),
        })
}

/// Writes the `CMake` package under `library/cmake/rola_plus_plus`.
///
/// The library the package names is the one beside these files, whose spelling this platform
/// decides: a build lays out `rorolala.so`, while a Linux package under `usr/lib` keeps the `lib`
/// prefix a linker expects. The targets file tries both names so one template serves both layouts.
///
/// # Errors
///
/// Reports [`Error::Io`] when a template cannot be read or a package file cannot be written, and
/// [`Error::Manifest`] when the workspace does not say its version.
pub fn write_package(workspace: &Workspace, library: &Path) -> Result<(), Error> {
    let version = workspace.version()?;
    let platform = Platform::current();
    let (shared, prefixed) = shared_names(platform);

    let import_library = platform.import_exported().map_or_else(String::new, |name| {
        format!("IMPORTED_IMPLIB \"${{CMAKE_CURRENT_LIST_DIR}}/../../{name}\"")
    });

    let replacements = [
        ("@ROLA_PLUS_PLUS_VERSION@", version.as_str()),
        ("@ROLA_PLUS_PLUS_SHARED@", shared),
        ("@ROLA_PLUS_PLUS_SHARED_PREFIXED@", prefixed),
        ("@ROLA_PLUS_PLUS_IMPORT_LIBRARY@", import_library.as_str()),
    ];

    let destination = library.join("cmake").join("rola_plus_plus");
    fs::create_directory(&destination)?;

    for file in CMAKE_FILES {
        let template = read_template(workspace, file)?;
        let rendered = replacements
            .iter()
            .fold(template, |text, (token, value)| text.replace(token, value));
        write_text(&destination.join(file), &rendered)?;
    }

    Ok(())
}

/// The shared library's own name and the name a Linux package gives it.
///
/// The two differ only on Unix: cargo writes `librorolala.so`, the export drops the prefix to give
/// a build directory one obvious name, and the Linux package keeps the linker's spelling beside the
/// static library.
fn shared_names(platform: Platform) -> (&'static str, &'static str) {
    match platform {
        Platform::Windows => ("rorolala.dll", "rorolala.dll"),
        Platform::Darwin => ("rorolala.dylib", "librorolala.dylib"),
        Platform::Unix => ("rorolala.so", "librorolala.so"),
    }
}

/// One template's text.
fn read_template(workspace: &Workspace, name: &str) -> Result<String, Error> {
    let path = workspace
        .root()
        .join(CMAKE_TEMPLATES)
        .join(format!("{name}.in"));

    std::fs::read_to_string(&path).map_err(|error| Error::Io {
        doing: "reading a CMake template",
        detail: format!("{}: {error}", path.display()),
    })
}

/// Writes one rendered package file.
fn write_text(path: &Path, text: &str) -> Result<(), Error> {
    std::fs::write(path, text).map_err(|error| Error::Io {
        doing: "writing a CMake package file",
        detail: format!("{}: {error}", path.display()),
    })
}
