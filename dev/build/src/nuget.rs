//! The two `NuGet` packages the C# bindings are handed over as.
//!
//! An export lays the bindings and the C ABI's library beside each other under `build/lib`, which is
//! what a program that reads the layout uses. A .NET consumer does not read a layout: it restores a
//! package, and the runtime finds the native library through that package's runtime assets. One
//! package therefore carries the managed assembly, and one per platform carries the native library
//! the assembly binds, so a release attaches the same two files a consumer restores.
//!
//! The native package is made for the platform the export was laid out for and for no other: what an
//! export holds is what this machine built. `win-x64` and `linux-x64` are the platforms one is made
//! for today, and `identifier_of` holds the ones left out, commented out with the reason.

use std::path::{Path, PathBuf};

use crate::{
    Error, dotnet, ops::publish::report, paths::Workspace, platform::Platform, toolchain::Tool,
};

/// The package that holds the managed bindings.
const MANAGED_ID: &str = "RolaSharp";

/// The prefix of the package that holds the native library for one platform.
const NATIVE_ID_PREFIX: &str = "RolaSharp.native";

/// The configuration the solution and both packages are built in.
const CONFIGURATION: &str = "Release";

/// Packs the bindings and, where this build is for one of the two supported platforms, the native
/// library beside them.
///
/// Both packages are written into the export's own `lib/`, which is the directory `publish` carries
/// into an installer and a release attaches as it stands. A build for another platform leaves the
/// native package out and says so, because there is no artifact to put in one.
///
/// # Errors
///
/// Reports metadata the workspace's manifest does not state, a `dotnet pack` that fails or cannot be
/// started, and a missing shared library when a native package was to be made.
pub fn pack_rsharp(workspace: &Workspace) -> Result<(), Error> {
    let version = workspace.version()?;
    let library = workspace.library_dir()?;

    report(&pack_managed(workspace, &library, &version)?)?;

    match native_identifier(workspace) {
        Some(identifier) => report(&pack_native(workspace, &library, &version, identifier)?)?,
        None => println!(
            "no native package: only win-x64 and linux-x64 get one, and this build is neither"
        ),
    }

    Ok(())
}

/// Packs the managed assembly, and answers where the package was written.
fn pack_managed(workspace: &Workspace, into: &Path, version: &str) -> Result<PathBuf, Error> {
    let project = workspace.rsharp_project().to_string_lossy().into_owned();
    let output = into.to_string_lossy().into_owned();

    // `--no-build` is what makes this the package of the assembly the export already laid down: a
    // pack that built again would write a second copy of it under a second timestamp.
    let arguments = vec![
        "pack".to_owned(),
        project,
        "-c".to_owned(),
        CONFIGURATION.to_owned(),
        "--no-build".to_owned(),
        "--no-restore".to_owned(),
        "-o".to_owned(),
        output,
        "-p:IsPackable=true".to_owned(),
        format!("-p:PackageId={MANAGED_ID}"),
        format!("-p:PackageVersion={version}"),
        format!("-p:Authors={}", workspace.publisher()?),
        format!("-p:Description={}", workspace.description()?),
        format!("-p:PackageLicenseExpression={}", workspace.license()?),
        format!("-p:PackageProjectUrl={}", workspace.homepage()?),
    ];

    Tool::dotnet().run(arguments)?;

    Ok(into.join(format!("{MANAGED_ID}.{version}.nupkg")))
}

/// Packs the native library for `identifier`, and answers where the package was written.
fn pack_native(
    workspace: &Workspace,
    into: &Path,
    version: &str,
    identifier: &str,
) -> Result<PathBuf, Error> {
    // The name taken is the one the runtime probes for `DllImport("rorolala")`, which is cargo's own
    // `librorolala.so` on Unix; the export's `lib/` holds the same library under the name a C
    // consumer links. A consequence is that a musl build has nothing to take, see `identifier_of`.
    let shared = workspace
        .release_dir()?
        .join(Platform::current().cargo_shared());

    if !shared.is_file() {
        return Err(Error::Missing {
            summary: format!(
                "the native library a package is made of is not there: {}",
                shared.display()
            ),
            advice: vec![
                "build the C ABI first, with `cargo dev lib`, then export again".to_owned(),
            ],
        });
    }

    let project = workspace.nuget_native_project();
    let project = project.to_string_lossy().into_owned();
    let shared = shared.to_string_lossy().into_owned();
    let output = into.to_string_lossy().into_owned();
    let library = format!("-p:NativeLibrary={shared}");
    let runtime = format!("-p:NativeRid={identifier}");

    // This project has nothing to fetch, but the restore is still tried with the sources taken away
    // first, the way every other .NET step here is: a machine with no feed must not wait for one.
    dotnet::restore(workspace, &[&project, &library, &runtime])?;

    let arguments = vec![
        "pack".to_owned(),
        project,
        "-c".to_owned(),
        CONFIGURATION.to_owned(),
        "--no-restore".to_owned(),
        "-o".to_owned(),
        output,
        library,
        runtime,
        format!("-p:PackageId={NATIVE_ID_PREFIX}.{identifier}"),
        format!("-p:PackageVersion={version}"),
        format!("-p:Authors={}", workspace.publisher()?),
        format!("-p:Description=The native library of the Rorolala C ABI for {identifier}"),
        format!("-p:PackageLicenseExpression={}", workspace.license()?),
        format!("-p:PackageProjectUrl={}", workspace.homepage()?),
    ];

    Tool::dotnet().run(arguments)?;

    Ok(into.join(format!("{NATIVE_ID_PREFIX}.{identifier}.{version}.nupkg")))
}

/// The runtime identifier a native package is made for, or none when this build is not one of the
/// two platforms one is made for.
#[must_use]
pub(crate) fn native_identifier(workspace: &Workspace) -> Option<&'static str> {
    identifier_of(
        workspace.target_triple().as_deref(),
        Platform::current(),
        std::env::consts::ARCH,
    )
}

/// The runtime identifier a build is for, read from the triple it names or from the machine.
///
/// The two are not interchangeable: a musl hand-off is built on a glibc machine, and it is not a
/// `linux-x64` package, since a musl build makes no shared library at all (see `ops/library.rs`).
fn identifier_of(triple: Option<&str>, platform: Platform, arch: &str) -> Option<&'static str> {
    if let Some(triple) = triple {
        return match triple {
            "x86_64-pc-windows-msvc" | "x86_64-pc-windows-gnu" => Some("win-x64"),
            "x86_64-unknown-linux-gnu" => Some("linux-x64"),
            // Not supported yet. Each needs an export built for its platform before a package of
            // its own can be made, and nothing in this workspace builds one.
            // "aarch64-unknown-linux-gnu" => Some("linux-arm64"),
            // "x86_64-apple-darwin" => Some("osx-x64"),
            // "aarch64-apple-darwin" => Some("osx-arm64"),
            // "aarch64-pc-windows-msvc" => Some("win-arm64"),
            // and every musl target, which makes no shared library at all (see `ops/library.rs`).
            _ => None,
        };
    }

    match (platform, arch) {
        (Platform::Windows, "x86_64") => Some("win-x64"),
        (Platform::Unix, "x86_64") => Some("linux-x64"),
        // The machine's own answer for the platforms commented out above.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::identifier_of;
    use crate::platform::Platform;

    #[test]
    fn only_the_two_supported_platforms_get_a_native_package() {
        // The machine's own answer, when a build names no triple.
        assert_eq!(
            identifier_of(None, Platform::Windows, "x86_64"),
            Some("win-x64")
        );
        assert_eq!(
            identifier_of(None, Platform::Unix, "x86_64"),
            Some("linux-x64")
        );

        // Every other platform and architecture is left out rather than guessed at.
        assert_eq!(identifier_of(None, Platform::Darwin, "aarch64"), None);
        assert_eq!(identifier_of(None, Platform::Unix, "aarch64"), None);
        assert_eq!(identifier_of(None, Platform::Windows, "aarch64"), None);
    }

    #[test]
    fn a_named_triple_is_read_rather_than_the_machine() {
        assert_eq!(
            identifier_of(Some("x86_64-unknown-linux-gnu"), Platform::Unix, "x86_64"),
            Some("linux-x64")
        );
        assert_eq!(
            identifier_of(Some("x86_64-pc-windows-msvc"), Platform::Windows, "x86_64"),
            Some("win-x64")
        );

        // A musl build makes no shared library, so it is not the glibc package under another name.
        assert_eq!(
            identifier_of(Some("x86_64-unknown-linux-musl"), Platform::Unix, "x86_64"),
            None
        );
        assert_eq!(
            identifier_of(Some("aarch64-unknown-linux-gnu"), Platform::Unix, "aarch64"),
            None
        );
    }
}
