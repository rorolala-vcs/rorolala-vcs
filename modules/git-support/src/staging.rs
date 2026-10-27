//! Moving a project's materials out of the working tree and back.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rola_define_format::{ContentHash, Lock, LockPath};
use serde::{Deserialize, Serialize};

use crate::content::{ContentSource, digest_of};
use crate::error::Error;

/// What a project keeps for the plugin rather than in its work.
#[derive(Debug, Clone)]
pub struct Private {
    /// The project-private directory, `.rola/git/`.
    root: PathBuf,
}

impl Private {
    /// The private area of the project rooted at `project_root`.
    #[must_use]
    pub fn of(project_root: &Path) -> Self {
        Self {
            root: project_root.join(".rola").join("git"),
        }
    }

    /// Where materials taken out of the working tree are kept.
    #[must_use]
    pub fn staging(&self) -> PathBuf {
        self.root.join("staging")
    }

    /// The file that says what is in the working tree.
    #[must_use]
    pub fn state_path(&self) -> PathBuf {
        self.root.join("state.toml")
    }
}

/// What a run last put into the working tree.
///
/// A path and its content are recorded together, so a later detach takes out exactly what this
/// plugin put in and a file a person made at a material's path is not mistaken for one of them.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    /// The materials in the working tree, each by the path it is placed at and the content it holds.
    #[serde(default)]
    pub attached: BTreeMap<String, String>,
}

impl State {
    /// Reads the state at `path`, or nothing when no run has written one.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the file cannot be read for another reason, and [`Error::State`]
    /// when what is there is not a state this build reads.
    pub fn read(path: &Path) -> Result<Self, Error> {
        match fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|error| Error::State(error.to_string())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(Error::Io(error)),
        }
    }

    /// Writes the state to `path`, making its directory.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the directory or the file cannot be written.
    pub fn write(&self, path: &Path) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(Error::Io)?;
        }

        let text = toml::to_string(self).map_err(|error| Error::State(error.to_string()))?;
        fs::write(path, text).map_err(Error::Io)
    }
}

/// Takes every material the state records out of the working tree and into the private area.
///
/// What is taken out is checked first: a path whose content is not what the state says was changed
/// after it was attached, so taking it out would lose the change. Nothing is moved when anything is
/// refused, so a run that cannot finish leaves the working tree as it found it.
///
/// # Errors
///
/// Returns [`Error::Content`] when a material is not what the state says, [`Error::Format`] when a
/// recorded path or content cannot be read, and [`Error::Io`] when a file cannot be moved or the
/// state cannot be written.
pub fn detach(private: &Private, working: &Path, state: &mut State) -> Result<(), Error> {
    let mut moving: Vec<(PathBuf, PathBuf)> = Vec::with_capacity(state.attached.len());

    for (recorded, content) in &state.attached {
        let path = LockPath::new(recorded).map_err(Error::Format)?;
        let source = working.join(disk_path(&path));

        if !source.exists() {
            // A material that is already gone is not one to take out: what was attached there was
            // moved by someone else, and the state is what is stale.
            continue;
        }

        let expected = ContentHash::from_token(content).map_err(Error::Format)?;
        let found = digest_of(&source)?;

        if found != expected {
            return Err(Error::Content {
                path: recorded.clone(),
                expected: content.clone(),
                found: found.to_token(),
            });
        }

        let target = private.staging().join(disk_path(&path));
        moving.push((source, target));
    }

    for (source, target) in moving {
        move_file(&source, &target)?;
    }

    state.attached.clear();
    state.write(&private.state_path())
}

/// Puts every material the lock names into the working tree, answering with the ones it could not.
///
/// A material already there with the content the lock names is left alone; one whose path holds
/// other content is refused rather than overwritten. What cannot be placed is reported one by one,
/// and what was placed is recorded, so a run that is interrupted can be retried.
///
/// # Errors
///
/// Returns [`Error::Io`] when the state cannot be written.
pub fn attach(
    private: &Private,
    working: &Path,
    lock: &Lock,
    source: &dyn ContentSource,
    state: &mut State,
) -> Result<Vec<(String, Error)>, Error> {
    let mut attached = BTreeMap::new();
    let mut failures = Vec::new();

    for material in lock.materials() {
        let path = material.path();
        let relative = disk_path(path);
        let target = working.join(&relative);

        match place(private, &relative, &target, material.content(), source) {
            Ok(()) => {
                attached.insert(path.as_str().to_owned(), material.content().to_token());
            }
            Err(error) => failures.push((path.as_str().to_owned(), error)),
        }
    }

    state.attached = attached;
    state.write(&private.state_path())?;

    Ok(failures)
}

/// Puts `only` of `lock`'s materials back, replacing whatever is at their paths.
///
/// `only` naming none means every material. What the lock does not name is not touched: a run
/// that was asked to restore something the lock holds no entry for has nothing to put back.
///
/// # Errors
///
/// Returns [`Error::Format`] when a path or content recorded in the state cannot be read.
pub fn restore(
    private: &Private,
    working: &Path,
    lock: &Lock,
    only: &[LockPath],
    source: &dyn ContentSource,
    state: &mut State,
) -> Result<Vec<(String, Error)>, Error> {
    let mut failures = Vec::new();

    for material in lock.materials() {
        if !only.is_empty() && !only.contains(material.path()) {
            continue;
        }

        let relative = disk_path(material.path());
        let target = working.join(&relative);

        match replace(private, &relative, &target, material.content(), source) {
            Ok(()) => {
                state.attached.insert(
                    material.path().as_str().to_owned(),
                    material.content().to_token(),
                );
            }
            Err(error) => failures.push((material.path().as_str().to_owned(), error)),
        }
    }

    Ok(failures)
}

/// Puts one material at `target`, replacing what is there.
fn replace(
    private: &Private,
    relative: &Path,
    target: &Path,
    content: ContentHash,
    source: &dyn ContentSource,
) -> Result<(), Error> {
    if target.exists() {
        if digest_of(target)? == content {
            return Ok(());
        }

        if target.is_dir() {
            return Err(Error::Taken {
                path: target.display().to_string(),
            });
        }

        fs::remove_file(target).map_err(Error::Io)?;
    }

    place(private, relative, target, content, source)
}

/// Puts one material at `target`, taking it out of the private area when it is already there.
fn place(
    private: &Private,
    relative: &Path,
    target: &Path,
    content: ContentHash,
    source: &dyn ContentSource,
) -> Result<(), Error> {
    let staged = private.staging().join(relative);

    if staged.is_file() && digest_of(&staged)? == content {
        return move_file(&staged, target);
    }

    if target.exists() {
        if digest_of(target)? == content {
            return Ok(());
        }

        return Err(Error::Taken {
            path: target.display().to_string(),
        });
    }

    source.write(content, target)?;

    let found = digest_of(target)?;
    if found != content {
        return Err(Error::Content {
            path: target.display().to_string(),
            expected: content.to_token(),
            found: found.to_token(),
        });
    }

    Ok(())
}

/// Where a lock's path lies on disk, which is one component per `/`.
fn disk_path(path: &LockPath) -> PathBuf {
    path.as_str().split('/').collect()
}

/// Moves `source` to `target`, making the directories above it.
fn move_file(source: &Path, target: &Path) -> Result<(), Error> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(Error::Io)?;
    }

    if target.exists() {
        fs::remove_file(target).map_err(Error::Io)?;
    }

    match fs::rename(source, target) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            // WORKAROUND: a staging area and a working tree can be on different volumes — a project
            // spread over mounts — where a rename is not one operation. The copy is the fallback,
            // and the original goes only once the copy is whole; a copy that cannot be made either
            // is the rename's failure as it was met.
            match fs::copy(source, target) {
                Ok(_) => {
                    fs::remove_file(source).map_err(Error::Io)?;
                    Ok(())
                }
                Err(_) => Err(Error::Io(rename_error)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use rola_define_format::{ContentHash, Lock, LockPath, Material};

    use super::{Private, State, attach, detach};
    use crate::content::{ContentSource, LocalStore, digest_of};
    use crate::error::Error;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rola-git-support-staging-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// A lock placing `paths` at one content each.
    fn lock(paths: &[(&str, [u8; 32])]) -> Lock {
        let materials = paths
            .iter()
            .map(|(path, digest)| {
                Material::new(
                    LockPath::new(path).unwrap(),
                    ContentHash::from_digest(*digest),
                )
            })
            .collect();

        Lock::new(vec![], materials).unwrap()
    }

    /// Puts `content` at `path` and brings the state in line with it.
    fn attach_text(working: &Path, path: &str, content: &[u8], state: &mut State) {
        let file = working.join(path.trim_start_matches('/'));
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, content).unwrap();
        let digest = digest_of(&file).unwrap();
        state.attached.insert(path.to_owned(), digest.to_token());
    }

    #[test]
    fn detaching_takes_materials_out_of_the_working_tree() {
        let dir = scratch("detach");
        let working = dir.join("work");
        let private = Private::of(&working);
        std::fs::create_dir_all(&working).unwrap();

        let mut state = State::default();
        attach_text(&working, "/Assets/hero.psd", b"hero", &mut state);

        detach(&private, &working, &mut state).unwrap();

        assert!(!working.join("Assets").join("hero.psd").exists());
        assert!(private.staging().join("Assets").join("hero.psd").is_file());
        assert!(state.attached.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_material_a_person_changed_is_not_taken_out() {
        let dir = scratch("changed");
        let working = dir.join("work");
        let private = Private::of(&working);
        std::fs::create_dir_all(working.join("Assets")).unwrap();
        std::fs::write(working.join("Assets").join("hero.psd"), b"edited").unwrap();

        let mut state = State::default();
        state.attached.insert(
            "Assets/hero.psd".to_owned(),
            ContentHash::from_digest([1; 32]).to_token(),
        );

        let error = detach(&private, &working, &mut state).unwrap_err();

        assert!(matches!(error, Error::Content { .. }), "{error}");
        assert!(working.join("Assets").join("hero.psd").is_file());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_material_that_is_already_gone_is_left_out_of_the_state() {
        let dir = scratch("gone");
        let working = dir.join("work");
        let private = Private::of(&working);
        std::fs::create_dir_all(&working).unwrap();

        let mut state = State::default();
        state.attached.insert(
            "Assets/hero.psd".to_owned(),
            ContentHash::from_digest([1; 32]).to_token(),
        );

        detach(&private, &working, &mut state).unwrap();

        assert!(state.attached.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn attaching_puts_the_lock_s_content_where_it_says() {
        let dir = scratch("attach");
        let working = dir.join("work");
        let private = Private::of(&working);
        std::fs::create_dir_all(&working).unwrap();

        let file = working.join("Assets").join("hero.psd");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, b"hero").unwrap();
        let digest = *digest_of(&file).unwrap().digest();

        // The content is moved away first, which is what a checkout after a detach looks like.
        let mut state = State::default();
        state.attached.insert(
            "Assets/hero.psd".to_owned(),
            digest_of(&file).unwrap().to_token(),
        );
        detach(&private, &working, &mut state).unwrap();

        let store = LocalStore::new(dir.join("store"));
        let object = store.object_path(ContentHash::from_digest(digest));
        std::fs::create_dir_all(object.parent().unwrap()).unwrap();
        std::fs::write(&object, b"hero").unwrap();

        let lock = lock(&[("/Assets/hero.psd", digest)]);
        let failures = attach(&private, &working, &lock, &store, &mut state).unwrap();

        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(std::fs::read(&file).unwrap(), b"hero");
        let recorded = state.attached.get("Assets/hero.psd").cloned();
        assert_eq!(recorded, Some(digest_of(&file).unwrap().to_token()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn attaching_from_staging_is_a_move_rather_than_a_fetch() {
        let dir = scratch("staged");
        let working = dir.join("work");
        let private = Private::of(&working);
        std::fs::create_dir_all(&working).unwrap();

        let mut state = State::default();
        attach_text(&working, "/Assets/hero.psd", b"hero", &mut state);
        let digest = *digest_of(&working.join("Assets").join("hero.psd"))
            .unwrap()
            .digest();
        detach(&private, &working, &mut state).unwrap();

        // A source that holds nothing: the staged copy is the only place the content is.
        let store = LocalStore::new(dir.join("store"));
        let lock = lock(&[("/Assets/hero.psd", digest)]);
        let failures = attach(&private, &working, &lock, &store, &mut state).unwrap();

        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(
            std::fs::read(working.join("Assets").join("hero.psd")).unwrap(),
            b"hero"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_place_holding_another_content_is_refused_rather_than_overwritten() {
        let dir = scratch("taken");
        let working = dir.join("work");
        let private = Private::of(&working);
        std::fs::create_dir_all(working.join("Assets")).unwrap();
        std::fs::write(working.join("Assets").join("hero.psd"), b"someone else").unwrap();

        let store = LocalStore::new(dir.join("store"));
        let lock = lock(&[("/Assets/hero.psd", [1; 32])]);
        let mut state = State::default();

        let failures = attach(&private, &working, &lock, &store, &mut state).unwrap();

        assert_eq!(failures.len(), 1);
        assert!(
            matches!(failures[0].1, Error::Taken { .. }),
            "{:?}",
            failures[0].1
        );
        assert_eq!(
            std::fs::read(working.join("Assets").join("hero.psd")).unwrap(),
            b"someone else"
        );
        assert!(state.attached.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_material_no_source_holds_is_reported_and_the_rest_are_placed() {
        let dir = scratch("partial");
        let working = dir.join("work");
        let private = Private::of(&working);
        std::fs::create_dir_all(&working).unwrap();

        let held = dir.join("held.bin");
        std::fs::write(&held, b"held").unwrap();
        let digest = *digest_of(&held).unwrap().digest();

        let store = LocalStore::new(dir.join("store"));
        let object = store.object_path(ContentHash::from_digest(digest));
        std::fs::create_dir_all(object.parent().unwrap()).unwrap();
        std::fs::write(&object, b"held").unwrap();

        let lock = lock(&[("/a.psd", digest), ("/b.psd", [3; 32])]);
        let mut state = State::default();
        let failures = attach(&private, &working, &lock, &store, &mut state).unwrap();

        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].0, "b.psd");
        assert!(matches!(failures[0].1, Error::Unavailable { .. }));
        assert!(state.attached.contains_key("a.psd"));
        assert!(!state.attached.contains_key("b.psd"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_state_round_trips_through_its_file() {
        let dir = scratch("state");
        let path = dir.join("state.toml");
        let mut state = State::default();
        state.attached.insert(
            "Assets/hero.psd".to_owned(),
            ContentHash::from_digest([4; 32]).to_token(),
        );

        state.write(&path).unwrap();
        let read = State::read(&path).unwrap();

        assert_eq!(read.attached, state.attached);

        let missing = State::read(&dir.join("nothing.toml")).unwrap();
        assert!(missing.attached.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_source_is_asked_only_when_the_content_is_nowhere_local() {
        /// A source that records what it was asked for and answers with nothing.
        #[derive(Default)]
        struct Nothing(std::cell::Cell<usize>);

        impl ContentSource for Nothing {
            fn write(&self, _content: ContentHash, _path: &std::path::Path) -> Result<(), Error> {
                self.0.set(self.0.get() + 1);

                Err(Error::Unavailable {
                    content: String::new(),
                })
            }
        }

        let dir = scratch("asked");
        let working = dir.join("work");
        let private = Private::of(&working);
        std::fs::create_dir_all(&working).unwrap();

        let mut state = State::default();
        attach_text(&working, "/Assets/hero.psd", b"hero", &mut state);
        let digest = *digest_of(&working.join("Assets").join("hero.psd"))
            .unwrap()
            .digest();
        let lock = lock(&[("/Assets/hero.psd", digest)]);

        // The file is already there with the content the lock names, so nothing is asked.
        let source = Nothing::default();
        let failures = attach(&private, &working, &lock, &source, &mut state).unwrap();

        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(source.0.get(), 0);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
