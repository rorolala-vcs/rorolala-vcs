//! Where a build works: the workspace it is in, and the directories its output goes to.
//!
//! Every directory is read once here rather than spelled where it is used, and every override is
//! checked before it is used: `export` and `clean` remove what these name, so a value that reaches
//! outside the workspace has to be refused before it names anything.

use std::path::{Component, Path, PathBuf};

use crate::Error;

/// The programs an export lays down, each an artifact of its own.
pub const PROGRAMS: [&str; 2] = ["rola", "rola-daemon"];

/// The programs that bring no completion scripts of their own: they are built and laid out beside
/// the others, and what an export copies for them differs.
pub const PROGRAMS_WITHOUT_COMPLETIONS: [&str; 2] = ["rola-jj", "rola-edit"];

/// The shells mingling writes a completion script for.
pub const COMPLETION_SHELLS: [&str; 4] = ["sh", "zsh", "fish", "ps1"];

/// The manifest that declares a workspace.
const MANIFEST: &str = "Cargo.toml";

/// Cargo's output directory, matching `.cargo/config.toml`.
const DEFAULT_TARGET_DIR: &str = ".cache/rs-target";

/// The .NET output root, matching `Directory.Build.props`.
const DEFAULT_CS_TARGET_DIR: &str = ".cache/cs-target";

/// Where an export lays the artifacts out.
const DEFAULT_BUILD_DIR: &str = "build";

/// Where a toolchain fetched rather than installed is kept.
const DEFAULT_TOOLCHAIN_DIR: &str = ".cache/toolchains";

/// The directory inside cargo's output that mingling writes completion scripts into.
const COMPLETION_DIR_IN_TARGET: &str = "mingling";

/// The directory inside cargo's output that release artifacts are written to.
const RELEASE_DIR_IN_TARGET: &str = "release";

/// The variable cargo reads the triple a build is for from.
const CARGO_BUILD_TARGET: &str = "CARGO_BUILD_TARGET";

/// The generated C header, inside the release directory.
const HEADER_IN_RELEASE: &str = "ffi_bindings/rorolala_ffi.h";

/// The completion scripts inside an export.
const SCRIPTS_IN_BUILD: &str = "scripts";

/// The Desktop program inside an export.
const DESKTOP_IN_BUILD: &str = "bin/desktop";

/// The Desktop program's project, from the workspace root.
const DESKTOP_PROJECT: &str = "app/desktop/Core/RorolalaDesktop.csproj";

/// The plugins that ship with the Desktop program.
const DESKTOP_PLUGINS: &str = "app/desktop/Plugins";

/// The launcher's project, from the workspace root.
const LAUNCHER_PROJECT: &str = "app/launcher/ROLALauncher.csproj";

/// The banners the launcher may play.
const LAUNCHER_BANNERS: &str = "res/desktop-launcher-banner";

/// The icon sets the Desktop may draw its file listing with.
const DESKTOP_ICON_SETS: &str = "res/desktop-icon-set";

/// The .NET side of the workspace as one solution.
const SOLUTION: &str = "RorolalaSharp.sln";

/// The C# bindings to the C ABI.
const RSHARP_PROJECT: &str = "modules/rola-sharp/RolaSharp.csproj";

/// The script that builds the Win32 window over the daemon into an export, on Windows.
const DAEMON_WINDOW_SCRIPT: &str = "app/daemon-win/scripts/daemon-win-build.ps1";

/// The `NuGet` configuration a restore is tried with first: the one with no sources.
const NUGET_OFFLINE: &str = "dev/configs/nuget.offline.config";

/// The workspace a build works in.
#[derive(Clone, Debug)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    /// Finds the workspace the program was started in, by walking up to the manifest that declares
    /// one.
    ///
    /// # Errors
    ///
    /// Reports [`Error::NoWorkspace`] when no directory above the current one declares a workspace,
    /// and [`Error::Io`] when the directory cannot be read.
    pub fn discover() -> Result<Self, Error> {
        let mut current = std::env::current_dir().map_err(|error| Error::Io {
            doing: "reading the current directory",
            detail: error.to_string(),
        })?;

        loop {
            if declares_workspace(&current.join(MANIFEST)) {
                return Ok(Self { root: current });
            }

            if !current.pop() {
                return Err(Error::NoWorkspace);
            }
        }
    }

    /// The directory the workspace's manifest sits in.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The workspace's own manifest.
    #[must_use]
    pub fn manifest_path(&self) -> PathBuf {
        self.root.join(MANIFEST)
    }

    /// Cargo's output directory, from `TARGET_DIR` when it is set.
    ///
    /// # Errors
    ///
    /// Reports [`Error::Directory`] when the override is not a directory inside the workspace.
    pub fn target_dir(&self) -> Result<PathBuf, Error> {
        self.directory("TARGET_DIR", DEFAULT_TARGET_DIR)
    }

    /// The architecture a build is for, when one is named: `CARGO_BUILD_TARGET`, the variable cargo
    /// reads the same value from.
    ///
    /// A build for the machine it runs on names no triple; one for another — a musl hand-off built on
    /// a glibc machine, or the reverse — names the triple cargo then writes its output under. Reading
    /// cargo's own variable rather than a second one is what keeps the two from disagreeing about
    /// where an artifact is.
    #[must_use]
    pub fn target_triple(&self) -> Option<String> {
        std::env::var(CARGO_BUILD_TARGET)
            .ok()
            .map(|triple| triple.trim().to_owned())
            .filter(|triple| !triple.is_empty())
    }

    /// Where cargo writes the release artifacts.
    ///
    /// # Errors
    ///
    /// As [`Self::target_dir`].
    pub fn release_dir(&self) -> Result<PathBuf, Error> {
        Ok(release_dir_in(
            self.target_dir()?,
            self.target_triple().as_deref(),
        ))
    }

    /// The generated C header, written by the root build script while the library builds.
    ///
    /// # Errors
    ///
    /// As [`Self::target_dir`].
    pub fn header(&self) -> Result<PathBuf, Error> {
        Ok(self.release_dir()?.join(HEADER_IN_RELEASE))
    }

    /// The directory mingling writes the completion scripts into while the programs compile.
    ///
    /// It is under the target directory and not under a triple inside it, because what mingling reads
    /// is the target directory `cargo metadata` answers, which names no triple: a completion script
    /// written for one architecture is a script the others can use as well.
    ///
    /// # Errors
    ///
    /// As [`Self::target_dir`].
    pub fn completion_dir(&self) -> Result<PathBuf, Error> {
        Ok(self.target_dir()?.join(COMPLETION_DIR_IN_TARGET))
    }

    /// The .NET output root, from `CS_TARGET_DIR` when it is set.
    ///
    /// # Errors
    ///
    /// Reports [`Error::Directory`] when the override is not a directory inside the workspace.
    pub fn cs_target_dir(&self) -> Result<PathBuf, Error> {
        self.directory("CS_TARGET_DIR", DEFAULT_CS_TARGET_DIR)
    }

    /// The directory an export lays the hand-off artifacts out in, from `BUILD_DIR` when it is set.
    ///
    /// # Errors
    ///
    /// Reports [`Error::Directory`] when the override is not a directory inside the workspace.
    pub fn build_dir(&self) -> Result<PathBuf, Error> {
        self.directory("BUILD_DIR", DEFAULT_BUILD_DIR)
    }

    /// Where a toolchain that is fetched rather than installed is kept, from `TOOLCHAIN_DIR` when it
    /// is set.
    ///
    /// # Errors
    ///
    /// Reports [`Error::Directory`] when the override is not a directory inside the workspace.
    pub fn toolchain_dir(&self) -> Result<PathBuf, Error> {
        self.directory("TOOLCHAIN_DIR", DEFAULT_TOOLCHAIN_DIR)
    }

    /// The completion scripts inside an export.
    ///
    /// # Errors
    ///
    /// As [`Self::build_dir`].
    pub fn scripts_dir(&self) -> Result<PathBuf, Error> {
        Ok(self.build_dir()?.join(SCRIPTS_IN_BUILD))
    }

    /// The Desktop program inside an export: where `rola desktop` reaches for it.
    ///
    /// # Errors
    ///
    /// As [`Self::build_dir`].
    pub fn desktop_dir(&self) -> Result<PathBuf, Error> {
        Ok(self.build_dir()?.join(DESKTOP_IN_BUILD))
    }

    /// The version the whole workspace is at, read from `[workspace.package]`.
    ///
    /// It is read from the manifest rather than asked of a program, because no program here knows
    /// it: the version is one number in the workspace's own manifest, and every package reads it
    /// there.
    ///
    /// # Errors
    ///
    /// Reports [`Error::Manifest`] when the manifest cannot be read or states no version.
    pub fn version(&self) -> Result<String, Error> {
        let manifest = self.read_manifest()?;

        version_of(&manifest).ok_or_else(|| Error::Manifest {
            path: self.manifest_path(),
            detail: "it states no `version` under `[workspace.package]`".to_owned(),
        })
    }

    /// What the project is published by: the first of the workspace's authors.
    ///
    /// The first is taken because a publisher is one name and the list is of people: what a reader
    /// is shown is who makes it.
    ///
    /// # Errors
    ///
    /// Reports [`Error::Manifest`] when the manifest cannot be read or states no author.
    pub fn publisher(&self) -> Result<String, Error> {
        let manifest = self.read_manifest()?;

        publisher_of(&manifest).ok_or_else(|| Error::Manifest {
            path: self.manifest_path(),
            detail: "it states no author under `[workspace.package]`".to_owned(),
        })
    }

    /// The plugins that ship with the Desktop program.
    ///
    /// A plugin is a project under `Plugins/`, one directory to one plugin, and the name of the
    /// directory is also the assembly's name — which is what says which files to take from the
    /// output it was built into. The tree is the one place a plugin is declared, so a directory
    /// added there is one a build lays down without being told about it.
    ///
    /// # Errors
    ///
    /// Reports [`Error::Io`] when the plugins directory cannot be read.
    pub fn desktop_plugins(&self) -> Result<Vec<DesktopPlugin>, Error> {
        let directory = self.root.join(DESKTOP_PLUGINS);
        let entries = std::fs::read_dir(&directory).map_err(|error| Error::Io {
            doing: "reading the Desktop plugins",
            detail: format!("{}: {error}", directory.display()),
        })?;

        let mut found = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| Error::Io {
                doing: "reading the Desktop plugins",
                detail: error.to_string(),
            })?;

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            if let Some(name) = path.file_name().and_then(std::ffi::OsStr::to_str) {
                let project = path.join(format!("{name}.csproj"));
                found.push((name.to_owned(), project.is_file()));
            }
        }

        Ok(plugin_names(&found)
            .into_iter()
            .map(|name| {
                let project = directory.join(&name).join(format!("{name}.csproj"));
                DesktopPlugin { name, project }
            })
            .collect())
    }

    /// The solution the .NET side of the workspace is built as.
    #[must_use]
    pub fn solution(&self) -> PathBuf {
        self.root.join(SOLUTION)
    }

    /// The Desktop program's project.
    #[must_use]
    pub fn desktop_project(&self) -> PathBuf {
        self.root.join(DESKTOP_PROJECT)
    }

    /// The launcher's project.
    #[must_use]
    pub fn launcher_project(&self) -> PathBuf {
        self.root.join(LAUNCHER_PROJECT)
    }

    /// The banners the launcher may play, taken from the tree rather than built.
    #[must_use]
    pub fn launcher_banners(&self) -> PathBuf {
        self.root.join(LAUNCHER_BANNERS)
    }

    /// The icon sets the Desktop may draw its file listing with.
    #[must_use]
    pub fn desktop_icon_sets(&self) -> PathBuf {
        self.root.join(DESKTOP_ICON_SETS)
    }

    /// The C# bindings to the C ABI.
    #[must_use]
    pub fn rsharp_project(&self) -> PathBuf {
        self.root.join(RSHARP_PROJECT)
    }

    /// The `NuGet` configuration a restore is tried with first.
    #[must_use]
    pub fn nuget_offline(&self) -> PathBuf {
        self.root.join(NUGET_OFFLINE)
    }

    /// The plugins directory itself, before anything in it is read.
    #[must_use]
    pub fn desktop_plugins_dir(&self) -> PathBuf {
        self.root.join(DESKTOP_PLUGINS)
    }

    /// The script that builds the daemon's window, on Windows.
    ///
    /// It is a script rather than a cargo build because the window is a C program of its own, with a
    /// resource script and the platform's compiler behind it — see `app/daemon-win/README.md`.
    #[must_use]
    pub fn daemon_window_script(&self) -> PathBuf {
        self.root.join(DAEMON_WINDOW_SCRIPT)
    }

    /// The manifest, read as text.
    fn read_manifest(&self) -> Result<String, Error> {
        let path = self.manifest_path();

        std::fs::read_to_string(&path).map_err(|error| Error::Manifest {
            path,
            detail: format!("it cannot be read: {error}"),
        })
    }

    /// A directory from an environment override or the default, checked and joined to the root.
    fn directory(&self, name: &'static str, default: &'static str) -> Result<PathBuf, Error> {
        let raw = std::env::var(name).ok();
        let relative = checked_relative(name, raw.as_deref(), default)?;

        Ok(self.root.join(relative))
    }
}

/// A plugin the Desktop program ships.
#[derive(Clone, Debug)]
pub struct DesktopPlugin {
    name: String,
    project: PathBuf,
}

impl DesktopPlugin {
    /// The plugin's name, which is also its assembly's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The plugin's project.
    #[must_use]
    pub fn project(&self) -> &Path {
        &self.project
    }
}

/// Whether the manifest at `path` is one that declares a workspace.
fn declares_workspace(path: &Path) -> bool {
    std::fs::read_to_string(path).is_ok_and(|manifest| declares_a_workspace(&manifest))
}

/// Whether `manifest` is one that declares a workspace.
///
/// The declaration is read as text rather than parsed: the question is only whether this is the
/// root of a workspace, and a manifest that says so in a form a parser would read differently is not
/// a case the build has to tell apart.
fn declares_a_workspace(manifest: &str) -> bool {
    manifest.contains("[workspace]")
}

/// The version `manifest` states under `[workspace.package]`, when it states one.
fn version_of(manifest: &str) -> Option<String> {
    let value = toml::from_str::<toml::Value>(manifest).ok()?;
    let version = value.get("workspace")?.get("package")?.get("version")?;

    Some(version.as_str()?.to_owned())
}

/// The first author `manifest` states under `[workspace.package]`, when it states one.
fn publisher_of(manifest: &str) -> Option<String> {
    let value = toml::from_str::<toml::Value>(manifest).ok()?;
    let authors = value.get("workspace")?.get("package")?.get("authors")?;

    Some(authors.as_array()?.first()?.as_str()?.to_owned())
}

/// The names of the plugins among `entries`, in the order a directory listing of them would be read
/// in.
///
/// An entry is a plugin when its directory holds a project named after it; anything else in there is
/// not one.
fn plugin_names(entries: &[(String, bool)]) -> Vec<String> {
    let mut names: Vec<String> = entries
        .iter()
        .filter(|(_, has_project)| *has_project)
        .map(|(name, _)| name.clone())
        .collect();
    names.sort();

    names
}

/// Where cargo writes release artifacts, given its target directory and the triple a build is for.
///
/// Cargo puts a named triple between the target directory and the profile; a build for the machine it
/// runs on has no such directory, so nothing is added.
fn release_dir_in(mut target: PathBuf, triple: Option<&str>) -> PathBuf {
    if let Some(triple) = triple {
        target.push(triple);
    }

    target.join(RELEASE_DIR_IN_TARGET)
}

/// A directory a build writes to, from an override or the default, refused when it does not stay
/// inside the workspace.
fn checked_relative(
    name: &'static str,
    raw: Option<&str>,
    default: &'static str,
) -> Result<PathBuf, Error> {
    // An empty override is the same as none: a variable set to nothing asked for nothing.
    let value = raw.filter(|value| !value.is_empty()).unwrap_or(default);

    let mut found_a_directory = false;
    for component in Path::new(value).components() {
        match component {
            Component::Normal(_) => found_a_directory = true,
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(Error::Directory {
                    name,
                    value: value.to_owned(),
                    reason: "it must stay inside the workspace",
                });
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(Error::Directory {
                    name,
                    value: value.to_owned(),
                    reason: "it must be a relative path",
                });
            }
        }
    }

    if !found_a_directory {
        return Err(Error::Directory {
            name,
            value: value.to_owned(),
            reason: "it must name a directory inside the workspace rather than the workspace itself",
        });
    }

    Ok(PathBuf::from(value))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        checked_relative, declares_a_workspace, plugin_names, publisher_of, release_dir_in,
        version_of,
    };

    #[test]
    fn a_manifest_declaring_a_workspace_is_recognised_and_another_is_not() {
        assert!(declares_a_workspace("[workspace]\nmembers = []\n"));
        assert!(declares_a_workspace(
            "[package]\nname = \"x\"\n\n[workspace]\n"
        ));

        // A dependency's own manifest, or a program's, is not a workspace root however long it is.
        assert!(!declares_a_workspace("[package]\nname = \"x\"\n"));
        assert!(!declares_a_workspace(
            "[dependencies]\nworkspace = { path = \"../workspace\" }\n"
        ));
    }

    #[test]
    fn the_version_and_the_publisher_are_read_from_the_workspace_section() {
        let manifest = "[package]\nname = \"x\"\nversion = \"9.9.9\"\n\n\
                        [workspace.package]\nlicense = \"MIT\"\nversion = \"0.1.0\"\n\
                        authors = [\"First Person\", \"Second Person\"]\n";

        assert_eq!(version_of(manifest).as_deref(), Some("0.1.0"));
        assert_eq!(publisher_of(manifest).as_deref(), Some("First Person"));

        // A manifest stating neither says so by stating nothing, rather than by guessing.
        assert_eq!(version_of("[package]\nname = \"x\"\n"), None);
        assert_eq!(publisher_of("[package]\nname = \"x\"\n"), None);
        assert_eq!(version_of("this is not a manifest"), None);
    }

    #[test]
    fn only_a_directory_holding_a_project_named_after_it_is_a_plugin() {
        let entries = [
            ("TerminalPlugin".to_owned(), true),
            ("FileSystemPlugin".to_owned(), true),
            ("Shared".to_owned(), false),
        ];

        // Sorted, so a directory listing is read in one order wherever it is read.
        assert_eq!(
            plugin_names(&entries),
            ["FileSystemPlugin", "TerminalPlugin"]
        );
        assert_eq!(plugin_names(&[]).len(), 0);
    }

    #[test]
    fn release_artifacts_sit_under_the_triple_only_when_a_build_names_one() {
        let target = PathBuf::from(".cache/rs-target");

        // A build for this machine: cargo writes straight into the target directory.
        assert_eq!(
            release_dir_in(target.clone(), None),
            PathBuf::from(".cache/rs-target/release")
        );

        // A build for another: cargo puts the triple in between, and so must the lookup.
        assert_eq!(
            release_dir_in(target, Some("x86_64-unknown-linux-musl")),
            PathBuf::from(".cache/rs-target/x86_64-unknown-linux-musl/release")
        );
    }

    #[test]
    fn an_override_has_to_name_a_directory_inside_the_workspace() {
        // Nothing, or nothing but spaces, is the default.
        assert_eq!(
            checked_relative("TARGET_DIR", None, ".cache/rs-target").ok(),
            Some(PathBuf::from(".cache/rs-target"))
        );
        assert_eq!(
            checked_relative("TARGET_DIR", Some(""), ".cache/rs-target").ok(),
            Some(PathBuf::from(".cache/rs-target"))
        );

        // A directory inside the workspace is taken as written.
        assert_eq!(
            checked_relative("BUILD_DIR", Some("build"), "build").ok(),
            Some(PathBuf::from("build"))
        );
        assert_eq!(
            checked_relative("BUILD_DIR", Some("./out/here"), "build").ok(),
            Some(PathBuf::from("./out/here"))
        );

        // What would reach outside it — or remove it — is refused.
        for refused in ["..", "build/../../etc", "/tmp/rola", "."] {
            assert!(
                checked_relative("BUILD_DIR", Some(refused), "build").is_err(),
                "`{refused}` should have been refused"
            );
        }

        // A Windows drive-relative value names something no relative path should reach either.
        #[cfg(windows)]
        assert!(checked_relative("BUILD_DIR", Some("C:out"), "build").is_err());
    }
}
