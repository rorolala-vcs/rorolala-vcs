//! `show-shape`: what the build works out about the tree before it does anything.
//!
//! Every step reads where it works from one place — the workspace, its directories, the platform's
//! artifact names, the programs an export lays down. This prints that place, so a disagreement is
//! seen as a value rather than met halfway through a build.

use std::io::Write as _;

use mingling::{
    Grouped, RenderResult, Routable,
    macros::{chain, command, renderer},
};

use crate::{
    Error, FailedBuild, Next,
    paths::{self, Workspace},
    platform::{self, Platform},
};

/// What the build works out about the tree before it does anything.
#[command(node = "show-shape")]
pub fn show_shape() -> StateShowShape {
    StateShowShape
}

/// The state the command starts in: nothing is named, since the answer is about the tree the
/// program was started in.
#[derive(Grouped)]
pub struct StateShowShape;

/// Result: the shape, one line per thing worked out.
#[derive(Grouped)]
pub struct ResultShowShape {
    /// The lines, in the order they are worth reading.
    lines: Vec<String>,
}

/// Works out the shape and answers with it.
#[chain]
pub fn handle_show_shape(_state: StateShowShape) -> Next {
    match shape() {
        Ok(lines) => ResultShowShape { lines }.to_render(),
        Err(error) => FailedBuild { error }.to_render(),
    }
}

/// Prints the shape.
#[renderer]
pub fn render_result_show_shape(result: ResultShowShape) -> RenderResult {
    let mut out = RenderResult::new();

    for line in &result.lines {
        writeln!(out, "{line}").ok();
    }

    out
}

/// The shape, as the lines that print it.
fn shape() -> Result<Vec<String>, Error> {
    let workspace = Workspace::discover()?;
    let platform = Platform::current();

    let import = platform.cargo_import().map_or_else(
        || "none: a linker is given the shared library itself".to_owned(),
        |cargo| format!("{cargo} -> {}", platform.import_exported().unwrap_or(cargo)),
    );

    let mut plugins: Vec<String> = workspace
        .desktop_plugins()?
        .iter()
        .map(|plugin| plugin.name().to_owned())
        .collect();

    if plugins.is_empty() {
        plugins.push("none".to_owned());
    }

    Ok(vec![
        format!("workspace root = {}", workspace.root().display()),
        format!("version = {}", workspace.version()?),
        format!("publisher = {}", workspace.publisher()?),
        String::new(),
        format!("platform = {}", platform.as_str()),
        format!("program suffix = {}", platform.exe_suffix()),
        format!(
            "shared library = {} -> {}",
            platform.cargo_shared(),
            platform.shared_exported()
        ),
        format!(
            "static library = {} -> {}",
            platform.cargo_static(),
            platform.static_exported()
        ),
        format!("import library = {import}"),
        format!(
            "archive platform = {}-{}",
            platform::uname_os(std::env::consts::OS),
            platform::uname_arch(std::env::consts::ARCH)
        ),
        String::new(),
        format!("target dir = {}", workspace.target_dir()?.display()),
        format!("release dir = {}", workspace.release_dir()?.display()),
        format!("header = {}", workspace.header()?.display()),
        format!("completion dir = {}", workspace.completion_dir()?.display()),
        format!("cs target dir = {}", workspace.cs_target_dir()?.display()),
        format!("build dir = {}", workspace.build_dir()?.display()),
        format!("toolchain dir = {}", workspace.toolchain_dir()?.display()),
        String::new(),
        format!("solution = {}", workspace.solution().display()),
        format!(
            "desktop project = {}",
            workspace.desktop_project().display()
        ),
        format!(
            "launcher project = {}",
            workspace.launcher_project().display()
        ),
        format!("rsharp project = {}", workspace.rsharp_project().display()),
        format!("nuget offline = {}", workspace.nuget_offline().display()),
        String::new(),
        format!("programs = {}", paths::PROGRAMS.join(" ")),
        format!(
            "programs without completions = {}",
            paths::PROGRAMS_WITHOUT_COMPLETIONS.join(" ")
        ),
        format!("completion shells = {}", paths::COMPLETION_SHELLS.join(" ")),
        format!("desktop plugins = {}", plugins.join(" ")),
    ])
}
