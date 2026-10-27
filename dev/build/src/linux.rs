//! What Linux is handed: the packages a distribution installs, and the `AppImage` a person runs.
//!
//! The packages are three parts of one product — the command line and daemon, the Desktop program,
//! and the C ABI — plus two ways to install all of them: `rorolala-full`, a meta package that
//! depends on the three, and `rorolala`, one package that holds what they hold. The two are
//! mutually exclusive, and an install of either is the whole product. The `AppImage` is the launcher
//! only: it carries `rola` because the launcher runs it, and nothing else, so a person who wants the
//! daemon has a package to install.
//!
//! What every artifact is made of is the export this crate already lays out. Nothing here compiles a
//! program: it stages what an export holds, writes the packager's configuration from the workspace's
//! own metadata, and runs the two pinned tools — nFPM for the three package formats, appimagetool
//! for the `AppImage`.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use serde::Serialize;

use crate::Error;
use crate::appimagetool;
use crate::fs;
use crate::nfpm;
use crate::ops::export::export_artifacts;
use crate::ops::publish::report;
use crate::paths::Workspace;
use crate::toolchain::Tool;

/// The package that holds the programs and the daemon.
const VCS: &str = "rorolala-vcs";

/// The package that holds the launcher and the Desktop program.
const DESKTOP: &str = "rorolala-desktop";

/// The package that holds the C ABI.
const DEV: &str = "rorolala-dev";

/// The meta package that names the three split packages.
const FULL: &str = "rorolala-full";

/// The package that holds all of `vcs`, `desktop` and `dev`, for one file that installs everything.
const ALL: &str = "rorolala";

/// The programs the command line package installs.
const PROGRAMS: [&str; 6] = [
    "rola",
    "rola-daemon",
    "rola-daemon-gui",
    "git-rola",
    "rola-jj",
    "rola-edit",
];

/// The release number every package is stamped with. It is not the project's version: a package
/// rebuilt from the same version is a new release of that version.
const RELEASE: &str = "1";

/// The icon sizes the Desktop's installer pictures are drawn at.
const ICON_SIZES: [u32; 6] = [16, 32, 64, 128, 256, 512];

/// What the `AppImage` runs: the launcher, which finds `rola` and the Desktop program beside it.
const APP_RUN: &str = "#!/bin/sh\nexec \"$APPDIR/usr/bin/desktop/ROLALauncher\" \"$@\"\n";

/// What a package says about itself, read from the workspace's own manifest.
struct Metadata {
    version: String,
    maintainer: String,
    vendor: String,
    homepage: String,
    description: String,
    license: String,
}

impl Metadata {
    fn read(workspace: &Workspace) -> Result<Self, Error> {
        Ok(Self {
            version: workspace.version()?,
            maintainer: workspace.maintainer()?,
            vendor: workspace.publisher()?,
            homepage: workspace.homepage()?,
            description: workspace.description()?,
            license: workspace.license()?,
        })
    }
}

/// One package, as the packager's configuration describes it.
struct PackageSpec {
    name: &'static str,
    description: String,
    contents: Vec<Content>,
    depends: BTreeMap<&'static str, Vec<String>>,
    recommends: BTreeMap<&'static str, Vec<String>>,
    conflicts: Vec<String>,
    replaces: Vec<String>,
}

/// One entry of a package's payload: a whole staged tree, or a link beside one.
#[derive(Clone, Serialize)]
struct Content {
    #[serde(skip_serializing_if = "Option::is_none")]
    src: Option<String>,
    dst: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<&'static str>,
}

impl Content {
    fn tree(source: &Path, destination: &str) -> Self {
        Self {
            src: Some(source.to_string_lossy().into_owned()),
            dst: destination.to_owned(),
            kind: Some("tree"),
        }
    }

    fn symlink(source: String, destination: String) -> Self {
        Self {
            src: Some(source),
            dst: destination,
            kind: Some("symlink"),
        }
    }
}

/// nFPM's configuration for one package.
#[derive(Serialize)]
struct NfpmConfig {
    name: &'static str,
    arch: &'static str,
    platform: &'static str,
    version: String,
    release: &'static str,
    section: &'static str,
    priority: &'static str,
    maintainer: String,
    description: String,
    vendor: String,
    homepage: String,
    license: String,
    contents: Vec<Content>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    conflicts: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    replaces: Vec<String>,
    scripts: Scripts,
    overrides: BTreeMap<&'static str, Override>,
    archlinux: ArchLinux,
}

/// The fields Arch's `.PKGINFO` carries and the other formats have no place for.
#[derive(Serialize)]
struct ArchLinux {
    packager: String,
    scripts: ArchLinuxScripts,
}

/// The scripts Arch runs on an upgrade; the other formats reuse the post-install one.
#[derive(Serialize)]
struct ArchLinuxScripts {
    postupgrade: String,
}

/// The scripts a package runs around an install, shared by every format.
#[derive(Serialize)]
struct Scripts {
    postinstall: String,
    postremove: String,
}

/// The fields a format overrides: the dependencies a distribution names its own way.
#[derive(Default, Serialize)]
struct Override {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    depends: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    recommends: Vec<String>,
}

/// Lays the export out, stages the packages, and builds every Linux artifact.
///
/// # Errors
///
/// Reports a missing packager, a missing toolchain, a failed export, a failed staging copy, or a
/// packager that ran and failed.
pub fn publish(workspace: &Workspace) -> Result<(), Error> {
    // The tools are asked for before the export rather than after it: what an export costs is
    // minutes of compiling, and a machine without a packager should say so first.
    let nfpm = Tool::at(nfpm::require(workspace)?);
    let appimagetool = Tool::at(appimagetool::require(workspace)?);
    let metadata = Metadata::read(workspace)?;

    export_artifacts(true)?;

    let build = workspace.build_dir()?;
    let staging = workspace.package_dir()?;
    fs::remove(&staging)?;
    fs::create_directory(&staging)?;

    let vcs = vcs_package(workspace, &build, &staging, &metadata)?;
    let desktop = desktop_package(workspace, &build, &staging, &metadata)?;
    let dev = dev_package(workspace, &build, &staging, &metadata)?;
    let full = full_package(workspace, &staging, &metadata)?;
    let all = all_package(workspace, &staging, [&vcs, &desktop, &dev])?;
    let packages = [vcs, desktop, dev, full, all];

    for package in &packages {
        for (packager, extension) in [("deb", "deb"), ("rpm", "rpm"), ("archlinux", "pkg.tar.zst")]
        {
            let target = build.join(format!(
                "{}-{}.{}",
                package.name, metadata.version, extension
            ));
            let configuration = staging
                .join("nfpm")
                .join(format!("{}-{packager}.yaml", package.name));

            write_configuration(&configuration, package, &metadata, workspace)?;
            fs::remove(&target)?;
            package_with(&nfpm, &configuration, packager, &target)?;
            report(&target)?;
        }
    }

    let appimage = build.join(format!("rola-launcher-{}.appimage", metadata.version));
    appimage_package(
        workspace,
        &build,
        &staging,
        &metadata,
        &appimagetool,
        &appimage,
    )?;
    report(&appimage)
}

fn vcs_package(
    workspace: &Workspace,
    build: &Path,
    staging: &Path,
    metadata: &Metadata,
) -> Result<PackageSpec, Error> {
    let root = staging.join(VCS);
    let bin = root.join("usr/lib/rorolala/bin");

    for program in PROGRAMS {
        fs::copy_file(&build.join("bin").join(program), &bin.join(program))?;
    }

    fs::copy_file(
        &workspace.root().join("dev/packaging/linux/rolad.service"),
        &root.join("usr/lib/systemd/user/rolad.service"),
    )?;

    write_desktop(
        workspace,
        &root.join("usr/share/applications/rorolala-daemon.desktop"),
        "rorolala-daemon.desktop",
        metadata,
    )?;
    copy_icons(workspace, &root)?;
    copy_completions(build, &root)?;
    copy_docs(workspace, &root, VCS)?;

    let mut contents = Vec::new();
    tree(&root, "usr/lib/rorolala/bin", &mut contents);
    tree(&root, "usr/lib/systemd/user", &mut contents);
    tree(&root, "usr/share/applications", &mut contents);
    tree(&root, "usr/share/icons", &mut contents);
    tree(&root, "usr/share/bash-completion", &mut contents);
    tree(&root, "usr/share/zsh", &mut contents);
    tree(&root, "usr/share/fish", &mut contents);
    tree(&root, &format!("usr/share/doc/{VCS}"), &mut contents);

    for program in PROGRAMS {
        contents.push(Content::symlink(
            format!("../lib/rorolala/bin/{program}"),
            format!("/usr/bin/{program}"),
        ));
    }

    Ok(PackageSpec {
        name: VCS,
        description: "The Rorolala command line, daemon and daemon window".to_owned(),
        contents,
        depends: dependencies(
            &["libc6", "libgcc-s1", "libgtk-4-1 (>= 4.10)"],
            &["glibc", "libgcc", "gtk4 >= 4.10"],
            &["glibc", "gcc-libs", "gtk4"],
        ),
        recommends: BTreeMap::new(),
        conflicts: Vec::new(),
        replaces: Vec::new(),
    })
}

fn desktop_package(
    workspace: &Workspace,
    build: &Path,
    staging: &Path,
    metadata: &Metadata,
) -> Result<PackageSpec, Error> {
    let root = staging.join(DESKTOP);
    fs::copy_directory(
        &build.join("bin/desktop"),
        &root.join("usr/lib/rorolala/bin/desktop"),
    )?;

    write_desktop(
        workspace,
        &root.join("usr/share/applications/rorolala-launcher.desktop"),
        "rorolala-launcher.desktop",
        metadata,
    )?;
    copy_docs(workspace, &root, DESKTOP)?;

    let mut contents = Vec::new();
    tree(&root, "usr/lib/rorolala/bin/desktop", &mut contents);
    tree(&root, "usr/share/applications", &mut contents);
    tree(&root, &format!("usr/share/doc/{DESKTOP}"), &mut contents);

    Ok(PackageSpec {
        name: DESKTOP,
        description: "The Rorolala launcher and Desktop window".to_owned(),
        contents,
        depends: desktop_dependencies(&metadata.version),
        recommends: BTreeMap::new(),
        conflicts: Vec::new(),
        replaces: Vec::new(),
    })
}

fn desktop_dependencies(version: &str) -> BTreeMap<&'static str, Vec<String>> {
    let (mut deb, mut rpm, mut arch) = desktop_native_dependencies();

    deb.insert(0, format!("{VCS} (= {version})"));
    rpm.insert(0, format!("{VCS} = {version}"));
    arch.insert(0, format!("{VCS}={version}"));

    dependencies_owned(deb, rpm, arch)
}

/// The libraries the self-contained Desktop still loads from the machine: the .NET runtime travels
/// with the program, but X11, Wayland, the fonts, ICU and OpenSSL do not.
fn desktop_native_dependencies() -> (Vec<String>, Vec<String>, Vec<String>) {
    (
        vec![
            "libc6".to_owned(),
            "libgcc-s1".to_owned(),
            "libstdc++6".to_owned(),
            "libicu72 | libicu74 | libicu76".to_owned(),
            "libfontconfig1".to_owned(),
            "libfreetype6".to_owned(),
            "libexpat1".to_owned(),
            "libbz2-1.0".to_owned(),
            "libpng16-16".to_owned(),
            "libbrotli1".to_owned(),
            "libx11-6".to_owned(),
            "libxext6".to_owned(),
            "libxrender1".to_owned(),
            "libxrandr2".to_owned(),
            "libxi6".to_owned(),
            "libxcursor1".to_owned(),
            "libice6".to_owned(),
            "libsm6".to_owned(),
            "libxcb1".to_owned(),
            "libxkbcommon0".to_owned(),
            "libwayland-client0".to_owned(),
            "libglib2.0-0".to_owned(),
            "libdbus-1-3".to_owned(),
            "libegl1".to_owned(),
            "libgl1".to_owned(),
            "libssl3".to_owned(),
            "zlib1g".to_owned(),
        ],
        vec![
            "glibc".to_owned(),
            "libgcc".to_owned(),
            "libstdc++".to_owned(),
            "libicu".to_owned(),
            "fontconfig".to_owned(),
            "freetype".to_owned(),
            "expat".to_owned(),
            "bzip2-libs".to_owned(),
            "libpng".to_owned(),
            "brotli".to_owned(),
            "libX11".to_owned(),
            "libXext".to_owned(),
            "libXrender".to_owned(),
            "libXrandr".to_owned(),
            "libXi".to_owned(),
            "libXcursor".to_owned(),
            "libICE".to_owned(),
            "libSM".to_owned(),
            "libxcb".to_owned(),
            "libxkbcommon".to_owned(),
            "wayland-libs-client".to_owned(),
            "glib2".to_owned(),
            "dbus-libs".to_owned(),
            "mesa-libEGL".to_owned(),
            "mesa-libGL".to_owned(),
            "openssl-libs".to_owned(),
            "zlib".to_owned(),
        ],
        vec![
            "glibc".to_owned(),
            "gcc-libs".to_owned(),
            "icu".to_owned(),
            "fontconfig".to_owned(),
            "freetype2".to_owned(),
            "expat".to_owned(),
            "bzip2".to_owned(),
            "libpng".to_owned(),
            "brotli".to_owned(),
            "libx11".to_owned(),
            "libxext".to_owned(),
            "libxrender".to_owned(),
            "libxrandr".to_owned(),
            "libxi".to_owned(),
            "libxcursor".to_owned(),
            "libice".to_owned(),
            "libsm".to_owned(),
            "libxcb".to_owned(),
            "libxkbcommon".to_owned(),
            "wayland".to_owned(),
            "glib2".to_owned(),
            "dbus".to_owned(),
            "mesa".to_owned(),
            "openssl".to_owned(),
            "zlib".to_owned(),
        ],
    )
}

fn dev_package(
    workspace: &Workspace,
    build: &Path,
    staging: &Path,
    _metadata: &Metadata,
) -> Result<PackageSpec, Error> {
    let root = staging.join(DEV);

    fs::copy_file(
        &build.join("lib/rorolala.h"),
        &root.join("usr/include/rorolala.h"),
    )?;
    fs::copy_file(
        &build.join("lib/rorolala.hpp"),
        &root.join("usr/include/rorolala.hpp"),
    )?;
    fs::copy_file(
        &build.join("lib/rorolala.so"),
        &root.join("usr/lib/librorolala.so"),
    )?;
    fs::copy_file(
        &build.join("lib/rorolala.a"),
        &root.join("usr/lib/librorolala.a"),
    )?;
    fs::copy_file(
        &build.join("lib/RolaSharp.dll"),
        &root.join("usr/lib/rorolala/RolaSharp.dll"),
    )?;
    copy_docs(workspace, &root, DEV)?;

    let mut contents = Vec::new();
    tree(&root, "usr/include", &mut contents);
    tree(&root, "usr/lib", &mut contents);
    tree(&root, &format!("usr/share/doc/{DEV}"), &mut contents);

    Ok(PackageSpec {
        name: DEV,
        description: "Headers and libraries for the Rorolala C ABI".to_owned(),
        contents,
        depends: dependencies(
            &["libc6", "libgcc-s1"],
            &["glibc", "libgcc"],
            &["glibc", "gcc-libs"],
        ),
        recommends: BTreeMap::new(),
        conflicts: Vec::new(),
        replaces: Vec::new(),
    })
}

fn full_package(
    workspace: &Workspace,
    staging: &Path,
    metadata: &Metadata,
) -> Result<PackageSpec, Error> {
    let root = staging.join(FULL);
    copy_docs(workspace, &root, FULL)?;

    let mut contents = Vec::new();
    tree(&root, &format!("usr/share/doc/{FULL}"), &mut contents);

    let version = format!("{}-{RELEASE}", metadata.version);
    let depends = BTreeMap::from([
        (
            "deb",
            vec![
                format!("{VCS} (= {version})"),
                format!("{DESKTOP} (= {version})"),
                format!("{DEV} (= {version})"),
            ],
        ),
        (
            "rpm",
            vec![
                format!("{VCS} = {version}"),
                format!("{DESKTOP} = {version}"),
                format!("{DEV} = {version}"),
            ],
        ),
        (
            "archlinux",
            vec![
                format!("{VCS}={version}"),
                format!("{DESKTOP}={version}"),
                format!("{DEV}={version}"),
            ],
        ),
    ]);

    Ok(PackageSpec {
        name: FULL,
        description: "The complete Rorolala suite, from the split packages".to_owned(),
        contents,
        depends,
        recommends: BTreeMap::new(),
        conflicts: vec![ALL.to_owned()],
        replaces: vec![ALL.to_owned()],
    })
}

/// The package that holds what the three split packages hold, so one file installs everything.
///
/// Its payload is the split packages' own: the staged trees are read where the split packages left
/// them, and only the documentation is the whole's own. It conflicts with all three, because the
/// same files cannot be owned twice, and with `rorolala-full`, because the two are two ways to
/// install the same product.
fn all_package(
    workspace: &Workspace,
    staging: &Path,
    parts: [&PackageSpec; 3],
) -> Result<PackageSpec, Error> {
    let root = staging.join(ALL);
    copy_docs(workspace, &root, ALL)?;

    let mut contents = Vec::new();
    for part in parts {
        for content in &part.contents {
            if content.dst.starts_with("/usr/share/doc/") {
                continue;
            }
            contents.push(content.clone());
        }
    }
    contents.push(Content::tree(
        &root.join(format!("usr/share/doc/{ALL}")),
        &format!("/usr/share/doc/{ALL}"),
    ));

    let depends = merged_dependencies(&[&parts[0].depends, &parts[1].depends, &parts[2].depends]);
    let conflicting = vec![
        VCS.to_owned(),
        DESKTOP.to_owned(),
        DEV.to_owned(),
        FULL.to_owned(),
    ];

    Ok(PackageSpec {
        name: ALL,
        description: "The complete Rorolala suite, in one package".to_owned(),
        contents,
        depends,
        recommends: BTreeMap::new(),
        conflicts: conflicting.clone(),
        replaces: conflicting,
    })
}

/// The dependencies of several packages as one, without the split packages' dependencies on each
/// other: an all-in-one package already holds them.
fn merged_dependencies(
    parts: &[&BTreeMap<&'static str, Vec<String>>],
) -> BTreeMap<&'static str, Vec<String>> {
    let mut merged: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();

    for part in parts {
        for (format, dependencies) in *part {
            let target = merged.entry(*format).or_default();

            for dependency in dependencies {
                if dependency.starts_with(VCS) || target.contains(dependency) {
                    continue;
                }
                target.push(dependency.clone());
            }
        }
    }

    merged
}

fn dependencies(deb: &[&str], rpm: &[&str], arch: &[&str]) -> BTreeMap<&'static str, Vec<String>> {
    dependencies_owned(
        deb.iter().map(|value| (*value).to_owned()).collect(),
        rpm.iter().map(|value| (*value).to_owned()).collect(),
        arch.iter().map(|value| (*value).to_owned()).collect(),
    )
}

fn dependencies_owned(
    deb: Vec<String>,
    rpm: Vec<String>,
    arch: Vec<String>,
) -> BTreeMap<&'static str, Vec<String>> {
    BTreeMap::from([("deb", deb), ("rpm", rpm), ("archlinux", arch)])
}

fn tree(root: &Path, relative: &str, contents: &mut Vec<Content>) {
    let source = root.join(relative);

    if source.is_dir() {
        contents.push(Content::tree(&source, &format!("/{relative}")));
    }
}

fn write_desktop(
    workspace: &Workspace,
    destination: &Path,
    template: &str,
    metadata: &Metadata,
) -> Result<(), Error> {
    let source = workspace.root().join("dev/packaging/linux").join(template);
    let text = read_text(&source)?.replace("@COMMENT@", &metadata.description);

    write_file(destination, &text, 0o644)
}

fn copy_icons(workspace: &Workspace, root: &Path) -> Result<(), Error> {
    for size in ICON_SIZES {
        let source = workspace
            .root()
            .join(format!("res/icons/installer/installer_ico_{size}.png"));

        if !source.is_file() {
            continue;
        }

        fs::copy_file(
            &source,
            &root.join(format!(
                "usr/share/icons/hicolor/{size}x{size}/apps/rorolala.png"
            )),
        )?;
    }

    Ok(())
}

fn copy_completions(build: &Path, root: &Path) -> Result<(), Error> {
    for (program, shell, destination) in [
        ("rola", "sh", "usr/share/bash-completion/completions/rola"),
        ("rola", "zsh", "usr/share/zsh/site-functions/_rola"),
        (
            "rola",
            "fish",
            "usr/share/fish/vendor_completions.d/rola.fish",
        ),
        (
            "rola-daemon",
            "sh",
            "usr/share/bash-completion/completions/rola-daemon",
        ),
        (
            "rola-daemon",
            "zsh",
            "usr/share/zsh/site-functions/_rola-daemon",
        ),
        (
            "rola-daemon",
            "fish",
            "usr/share/fish/vendor_completions.d/rola-daemon.fish",
        ),
    ] {
        let source = build
            .join("scripts")
            .join(program)
            .join(format!("{program}-completion.{shell}"));

        if source.is_file() {
            fs::copy_file(&source, &root.join(destination))?;
        }
    }

    Ok(())
}

fn copy_docs(workspace: &Workspace, root: &Path, package: &str) -> Result<(), Error> {
    let docs = root.join("usr/share/doc").join(package);

    fs::copy_file(&workspace.root().join("README.md"), &docs.join("README.md"))?;
    fs::copy_file(&workspace.root().join("LICENSE"), &docs.join("LICENSE"))?;
    fs::copy_file(&workspace.root().join("LICENSE"), &docs.join("copyright"))?;

    Ok(())
}

fn write_configuration(
    path: &Path,
    package: &PackageSpec,
    metadata: &Metadata,
    workspace: &Workspace,
) -> Result<(), Error> {
    let mut overrides = BTreeMap::new();
    for format in ["deb", "rpm", "archlinux"] {
        overrides.insert(
            format,
            Override {
                depends: package.depends.get(format).cloned().unwrap_or_default(),
                recommends: package.recommends.get(format).cloned().unwrap_or_default(),
            },
        );
    }

    let refresh = workspace
        .root()
        .join("dev/packaging/linux/scripts/refresh.sh")
        .to_string_lossy()
        .into_owned();

    let configuration = NfpmConfig {
        name: package.name,
        arch: std::env::consts::ARCH,
        platform: "linux",
        version: metadata.version.clone(),
        release: RELEASE,
        section: "devel",
        priority: "optional",
        maintainer: metadata.maintainer.clone(),
        description: package.description.clone(),
        vendor: metadata.vendor.clone(),
        homepage: metadata.homepage.clone(),
        license: metadata.license.clone(),
        contents: package.contents.clone(),
        conflicts: package.conflicts.clone(),
        replaces: package.replaces.clone(),
        scripts: Scripts {
            postinstall: refresh.clone(),
            postremove: refresh.clone(),
        },
        overrides,
        archlinux: ArchLinux {
            packager: metadata.maintainer.clone(),
            scripts: ArchLinuxScripts {
                postupgrade: refresh,
            },
        },
    };

    let yaml = serde_yaml::to_string(&configuration).map_err(|error| Error::Io {
        doing: "writing a packager's configuration",
        detail: error.to_string(),
    })?;

    write_file(path, &yaml, 0o644)
}

fn package_with(
    nfpm: &Tool,
    configuration: &Path,
    packager: &str,
    target: &Path,
) -> Result<(), Error> {
    let configuration = configuration.to_string_lossy().into_owned();
    let target = target.to_string_lossy().into_owned();

    nfpm.run([
        "package",
        "--config",
        configuration.as_str(),
        "--packager",
        packager,
        "--target",
        target.as_str(),
    ])
}

fn appimage_package(
    workspace: &Workspace,
    build: &Path,
    staging: &Path,
    metadata: &Metadata,
    tool: &Tool,
    target: &Path,
) -> Result<(), Error> {
    let appdir = staging.join("appimage/AppDir");
    fs::remove(&appdir)?;

    fs::copy_file(&build.join("bin/rola"), &appdir.join("usr/bin/rola"))?;
    fs::copy_directory(&build.join("bin/desktop"), &appdir.join("usr/bin/desktop"))?;

    write_file(&appdir.join("AppRun"), APP_RUN, 0o755)?;
    write_desktop(
        workspace,
        &appdir.join("rorolala-launcher.desktop"),
        "appimage.desktop",
        metadata,
    )?;
    fs::copy_file(
        &workspace
            .root()
            .join("res/icons/installer/installer_ico_256.png"),
        &appdir.join("rorolala.png"),
    )?;
    symlink("rorolala.png", &appdir.join(".DirIcon"))?;

    fs::remove(target)?;

    let appdir = appdir.to_string_lossy().into_owned();
    let target = target.to_string_lossy().into_owned();

    tool.run_with_env(
        &[
            ("ARCH", OsStr::new(std::env::consts::ARCH)),
            ("APPIMAGE_EXTRACT_AND_RUN", OsStr::new("1")),
        ],
        ["--no-appstream", appdir.as_str(), target.as_str()],
    )
}

fn read_text(path: &Path) -> Result<String, Error> {
    std::fs::read_to_string(path).map_err(|error| Error::Io {
        doing: "reading a package template",
        detail: format!("{}: {error}", path.display()),
    })
}

fn write_file(path: &Path, contents: &str, mode: u32) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_directory(parent)?;
    }

    std::fs::write(path, contents).map_err(|error| Error::Io {
        doing: "writing a package file",
        detail: format!("{}: {error}", path.display()),
    })?;

    let mut permissions = std::fs::metadata(path)
        .map_err(|error| Error::Io {
            doing: "reading a package file's mode",
            detail: format!("{}: {error}", path.display()),
        })?
        .permissions();
    permissions.set_mode(mode);

    std::fs::set_permissions(path, permissions).map_err(|error| Error::Io {
        doing: "setting a package file's mode",
        detail: format!("{}: {error}", path.display()),
    })
}

fn symlink(target: &str, path: &Path) -> Result<(), Error> {
    std::os::unix::fs::symlink(target, path).map_err(|error| Error::Io {
        doing: "linking a package file",
        detail: format!("{}: {error}", path.display()),
    })
}
