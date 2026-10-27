//! The paths a `restore` names, read the way git reads a pathspec.
//!
//! A name is a path relative to the directory the run is made in: a material's own path, a
//! directory — everything under it — or `.` for everything under the current directory. A name
//! written with a leading `/` is relative to the project's root instead, and a name written the way
//! [`check`](crate::check) prints it — from the work tree's root — is read that way when nothing
//! else matches it, since that is the spelling a person copies out of a `check`.
//!
//! Reading the names is a pure business: what a name means is decided from the paths alone, so the
//! reading is tested here rather than through the program.

use std::path::Path;

use rola_define_format::{Lock, LockPath};

/// Why a name selected nothing.
#[derive(Debug)]
pub enum Refusal {
    /// No material the lock names is under the name.
    NoMatch(String),
    /// The name leaves the project: nothing outside it is a material.
    Outside(String),
    /// The name is not a path a lock could name.
    Invalid(String),
}

/// The names a restore is given, and whether each selects something.
pub struct Choice {
    /// The materials the names selected, in the order the lock lists them.
    pub materials: Vec<LockPath>,
}

/// The materials of `lock` that `specs` select.
///
/// `base` is the directory the run is made in, relative to the project's root and `/`-separated;
/// `prefix` is the project as the work tree names it, `.` when the project is the work tree's root.
///
/// A name that selects nothing is refused rather than passed over: a restore that mistypes one name
/// restores nothing at all, the way a `git restore` whose pathspec matched no file does.
///
/// # Errors
///
/// Returns a [`Refusal`] for a name no material is under, one that leaves the project, and one that
/// is not a path a lock could name.
pub fn select(lock: &Lock, base: &Path, prefix: &str, specs: &[String]) -> Result<Choice, Refusal> {
    // Each name is settled by the first reading of it that the lock has something under: a name is
    // meant as it reads from the run's directory, and the other readings are only there for a name
    // that would otherwise match nothing.
    let mut settled = Vec::with_capacity(specs.len());

    for spec in specs {
        let candidates = wanted(spec, base, prefix)?;

        let Some(reached) = candidates.iter().find(|candidate| {
            lock.materials()
                .iter()
                .any(|material| covers(candidate, material.path().as_str()))
        }) else {
            return Err(Refusal::NoMatch(spec.clone()));
        };

        settled.push(reached.clone());
    }

    let materials = lock
        .materials()
        .iter()
        .filter(|material| {
            settled
                .iter()
                .any(|candidate| covers(candidate, material.path().as_str()))
        })
        .map(|material| material.path().clone())
        .collect();

    Ok(Choice { materials })
}

/// The project-relative paths `spec` may name, in the order they are tried.
///
/// The first is what the name means from the run's directory, which is what a name means to git. The
/// others are what the name can still reasonably mean: a name written from the project's root, or
/// one written the way `check` prints it from the work tree's root. A path is chosen by the material
/// it matches rather than by which reading found it, so a name that two readings could give is
/// settled by the lock.
///
/// # Errors
///
/// Returns a [`Refusal`] for a name that leaves the project or is not a path.
fn wanted(spec: &str, base: &Path, prefix: &str) -> Result<Vec<String>, Refusal> {
    let name = spec.replace('\\', "/");

    // A leading `/` is the project's root, the way a lock writes a path.
    if let Some(rooted) = name.strip_prefix('/') {
        return Ok(vec![read(Path::new(""), rooted)?]);
    }

    let mut candidates = vec![read(base, &name)?];

    // A name written from the project's root still names the material when the run is above it in
    // the project, and a name written from the work tree's root — what `check` prints — still names
    // it when the project is below that root. A reading that would leave the project is no reading
    // at all here: the run's own reading has already decided what the name means.
    if let Ok(from_project) = read(Path::new(""), &name) {
        candidates.push(from_project);
    }

    if prefix != "." {
        let project = prefix.replace('\\', "/");
        let project = project.trim_end_matches('/');

        if (name == project || name.starts_with(&format!("{project}/")))
            && let Ok(under) = read(
                Path::new(""),
                name.strip_prefix(project)
                    .unwrap_or_default()
                    .trim_start_matches('/'),
            )
        {
            candidates.push(under);
        }
    }

    candidates.dedup();

    Ok(candidates)
}

/// `base` joined with `spec`, with `.` and `..` read, as a project-relative `/`-separated path.
///
/// The empty string is the project's root itself.
///
/// # Errors
///
/// Returns [`Refusal::Outside`] when a `..` climbs above the project, and [`Refusal::Invalid`] when a
/// component is not one a path may hold.
fn read(base: &Path, spec: &str) -> Result<String, Refusal> {
    let mut parts: Vec<String> = base
        .components()
        .filter_map(|part| match part {
            std::path::Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();

    for part in spec.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(Refusal::Outside(spec.to_owned()));
                }
            }
            other => {
                if other.contains(':') || other.contains('\\') {
                    return Err(Refusal::Invalid(spec.to_owned()));
                }

                parts.push(other.to_owned());
            }
        }
    }

    Ok(parts.join("/"))
}

/// Whether `candidate` names `material` or something above it.
///
/// A candidate is a directory as well as a path: one the material sits under selects the material,
/// which is how a directory name restores everything beneath it.
fn covers(candidate: &str, material: &str) -> bool {
    candidate.is_empty()
        || candidate == material
        || material
            .strip_prefix(candidate)
            .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use rola_define_format::{ContentHash, Lock, LockPath, Material};

    use super::{Refusal, select};

    /// A lock naming `paths` at one content each.
    fn lock(paths: &[&str]) -> Lock {
        let materials = paths
            .iter()
            .map(|path| {
                Material::new(
                    LockPath::new(path).unwrap(),
                    ContentHash::from_digest([1; 32]),
                )
            })
            .collect();

        Lock::new(vec![], materials).unwrap()
    }

    /// The paths `specs` select, as the lock lists them.
    fn chosen(lock: &Lock, base: &str, prefix: &str, specs: &[&str]) -> Vec<String> {
        let specs: Vec<String> = specs.iter().map(|spec| (*spec).to_owned()).collect();

        select(lock, std::path::Path::new(base), prefix, &specs)
            .unwrap()
            .materials
            .iter()
            .map(|path| path.as_str().to_owned())
            .collect()
    }

    #[test]
    fn a_dotted_name_is_everything_under_the_run() {
        let lock = lock(&["Assets/hero.psd", "Assets/bg.psd", "Source/c.fbx"]);

        assert_eq!(
            chosen(&lock, "", ".", &["."]),
            ["Assets/bg.psd", "Assets/hero.psd", "Source/c.fbx"]
        );
        assert_eq!(
            chosen(&lock, "Assets", ".", &["."]),
            ["Assets/bg.psd", "Assets/hero.psd"]
        );
    }

    #[test]
    fn a_directory_name_is_everything_under_it() {
        let lock = lock(&["Assets/hero.psd", "Assets/bg.psd", "Source/c.fbx"]);

        assert_eq!(
            chosen(&lock, "", ".", &["Assets"]),
            ["Assets/bg.psd", "Assets/hero.psd"]
        );
        assert_eq!(
            chosen(&lock, "", ".", &["Assets/"]),
            ["Assets/bg.psd", "Assets/hero.psd"]
        );
        assert_eq!(chosen(&lock, "Assets", ".", &["bg.psd"]), ["Assets/bg.psd"]);
    }

    #[test]
    fn a_material_is_named_by_its_own_path() {
        let lock = lock(&["Assets/hero.psd", "Source/c.fbx"]);

        assert_eq!(
            chosen(&lock, "", ".", &["Assets/hero.psd"]),
            ["Assets/hero.psd"]
        );
        assert_eq!(
            chosen(&lock, "Assets", ".", &["hero.psd"]),
            ["Assets/hero.psd"]
        );
        assert_eq!(
            chosen(&lock, "Assets", ".", &["../Source/c.fbx"]),
            ["Source/c.fbx"]
        );
    }

    #[test]
    fn a_leading_slash_is_the_projects_root() {
        let lock = lock(&["Assets/hero.psd"]);

        assert_eq!(
            chosen(&lock, "Assets", ".", &["/Assets/hero.psd"]),
            ["Assets/hero.psd"]
        );
        assert_eq!(
            chosen(&lock, "Assets", ".", &["/Assets"]),
            ["Assets/hero.psd"]
        );
    }

    #[test]
    fn a_name_check_printed_from_the_work_tree_still_names_the_material() {
        let lock = lock(&["Assets/hero.psd", "Source/c.fbx"]);

        // `check` in a project at `sub/` prints `sub/Assets/hero.psd`, and a run from inside the
        // project copies that name out of it.
        assert_eq!(
            chosen(&lock, "", "sub", &["sub/Assets/hero.psd"]),
            ["Assets/hero.psd"]
        );
        assert_eq!(
            chosen(&lock, "", "sub", &["sub"]),
            ["Assets/hero.psd", "Source/c.fbx"]
        );
    }

    #[test]
    fn names_are_read_together_and_each_has_to_match_something() {
        let lock = lock(&["Assets/hero.psd", "Assets/bg.psd", "Source/c.fbx"]);

        assert_eq!(
            chosen(&lock, "", ".", &["Assets/hero.psd", "Source"]),
            ["Assets/hero.psd", "Source/c.fbx"]
        );
        // The same name twice selects the material once.
        assert_eq!(
            chosen(&lock, "", ".", &["Assets", "."]),
            ["Assets/bg.psd", "Assets/hero.psd", "Source/c.fbx"]
        );

        let refused = select(
            &lock,
            std::path::Path::new(""),
            ".",
            &["Assets".to_owned(), "nowhere".to_owned()],
        );

        assert!(matches!(refused, Err(Refusal::NoMatch(name)) if name == "nowhere"));
    }

    #[test]
    fn a_name_that_climbs_out_of_the_project_is_refused() {
        let lock = lock(&["Assets/hero.psd"]);

        let refused = select(&lock, std::path::Path::new(""), ".", &["..".to_owned()]);

        assert!(matches!(refused, Err(Refusal::Outside(name)) if name == ".."));
    }
}
