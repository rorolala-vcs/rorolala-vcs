//! Moving a project's materials out of the working tree and back.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rola_define_format::{ContentHash, Lock, LockPath, MaterialKind};
use serde::{Deserialize, Deserializer, Serialize};

use crate::content::{ContentSource, digest_of};
use crate::error::Error;
use crate::tree;

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
/// A path and what was placed there are recorded together, so a later detach takes out exactly what
/// this plugin put in and a file a person made at a material's path is not mistaken for one of them.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    /// The materials in the working tree, each by the path it is placed at and what holds there.
    #[serde(default)]
    pub attached: BTreeMap<String, Placed>,
}

/// What a run last placed at a material's path.
///
/// The kind is kept beside the content because a detach reads the state and no lock, and a tree is
/// taken out and checked differently from a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Placed {
    /// The content placed there, as a lock writes it.
    pub content: String,
    /// The kind of thing placed there, as a lock names it.
    pub kind: String,
}

impl Placed {
    /// A material of `kind` holding `content`.
    #[must_use]
    pub fn new(content: impl Into<String>, kind: MaterialKind) -> Self {
        Self {
            content: content.into(),
            kind: kind.token().to_owned(),
        }
    }

    /// The kind placed there.
    ///
    /// # Errors
    ///
    /// Returns [`Error::State`] for a kind this build does not read, which is a state written by
    /// something else rather than a material.
    pub fn kind(&self) -> Result<MaterialKind, Error> {
        MaterialKind::from_token(&self.kind).ok_or_else(|| {
            Error::State(format!(
                "`{}` is not a material kind this build reads",
                self.kind
            ))
        })
    }

    /// The content placed there, read as a lock writes one.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Format`] when the recorded content is not one a lock could write.
    pub fn content(&self) -> Result<ContentHash, Error> {
        ContentHash::from_token(&self.content).map_err(Error::Format)
    }
}

impl<'de> Deserialize<'de> for Placed {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        /// What a state may hold at a path: the content alone, as states written before kinds were
        /// recorded hold it, or the content and the kind.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            /// The content alone, which is what a state written before kinds were recorded holds.
            Content(String),
            /// The content and the kind.
            Detail {
                /// The content placed there.
                content: String,
                /// The kind placed there.
                #[serde(default = "file_kind")]
                kind: String,
            },
        }

        /// The kind a state written before kinds were recorded is read as.
        fn file_kind() -> String {
            MaterialKind::File.token().to_owned()
        }

        Ok(match Repr::deserialize(deserializer)? {
            Repr::Content(content) => Self::new(content, MaterialKind::File),
            Repr::Detail { content, kind } => Self { content, kind },
        })
    }
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
/// What is taken out is checked first: a file whose content is not what the state says, and a tree
/// whose paths are not the structure it was placed from, were changed after they were attached, so
/// taking them out would lose the change. Nothing is moved when anything is refused, so a run that
/// cannot finish leaves the working tree as it found it.
///
/// # Errors
///
/// Returns [`Error::Content`] or [`Error::TreeContent`] when a material is not what the state says,
/// [`Error::Format`] when a recorded path or content cannot be read, [`Error::Structure`] when a
/// tree's content is not a structure, and [`Error::Io`] when a material cannot be moved or the
/// state cannot be written.
pub fn detach(
    private: &Private,
    working: &Path,
    state: &mut State,
    source: &dyn ContentSource,
) -> Result<(), Error> {
    /// What is to be moved, and where to.
    enum Moving {
        /// A file, moved on its own.
        File(PathBuf, PathBuf),
        /// A tree, moved with the directory it is.
        Tree(PathBuf, PathBuf),
    }

    let mut moving: Vec<Moving> = Vec::with_capacity(state.attached.len());

    for (recorded, placed) in &state.attached {
        let path = LockPath::new(recorded).map_err(Error::Format)?;
        let on_disk = working.join(disk_path(&path));

        if !on_disk.exists() {
            // A material that is already gone is not one to take out: what was attached there was
            // moved by someone else, and the state is what is stale.
            continue;
        }

        let target = private.staging().join(disk_path(&path));

        match placed.kind()? {
            MaterialKind::File => {
                let expected = placed.content()?;
                let found = digest_of(&on_disk)?;

                if found != expected {
                    return Err(Error::Content {
                        path: recorded.clone(),
                        expected: placed.content.clone(),
                        found: found.to_token(),
                    });
                }

                moving.push(Moving::File(on_disk, target));
            }
            MaterialKind::Tree => {
                if !on_disk.is_dir() {
                    return Err(Error::TreeContent {
                        path: recorded.clone(),
                        at: ".".to_owned(),
                    });
                }

                let structure = tree::structure(source, placed.content()?)?;

                if let Some(first) = tree::differences(&on_disk, &structure)?.first() {
                    return Err(Error::TreeContent {
                        path: recorded.clone(),
                        at: first.at.clone(),
                    });
                }

                moving.push(Moving::Tree(on_disk, target));
            }
        }
    }

    for material in moving {
        match material {
            Moving::File(from, to) => move_file(&from, &to)?,
            Moving::Tree(from, to) => tree::move_directory(&from, &to)?,
        }
    }

    state.attached.clear();
    state.write(&private.state_path())
}

/// Puts every material the lock names into the working tree, answering with the ones it could not.
///
/// A file already there with the content the lock names is left alone; one whose path holds other
/// content is refused rather than overwritten. A tree is placed as the directory its structure
/// names, and one already there that is exactly that structure is left alone, so a run made twice
/// places nothing the second time. What cannot be placed is reported one by one, and what was
/// placed is recorded, so a run that is interrupted can be retried.
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

        let placed = match material.kind() {
            MaterialKind::File => {
                place_file(private, &relative, &target, material.content(), source)
            }
            MaterialKind::Tree => {
                place_tree(private, &relative, &target, material.content(), source)
            }
        };

        match placed {
            Ok(()) => {
                attached.insert(
                    path.as_str().to_owned(),
                    Placed::new(material.content().to_token(), material.kind()),
                );
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
/// that was asked to restore something the lock holds no entry for has nothing to put back. A tree
/// is written back as the structure the lock names, over the paths it names, and paths the
/// structure does not name are left where they are.
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

        let placed = match material.kind() {
            MaterialKind::File => {
                replace_file(private, &relative, &target, material.content(), source)
            }
            MaterialKind::Tree => {
                replace_tree(private, &relative, &target, material.content(), source)
            }
        };

        match placed {
            Ok(()) => {
                state.attached.insert(
                    material.path().as_str().to_owned(),
                    Placed::new(material.content().to_token(), material.kind()),
                );
            }
            Err(error) => failures.push((material.path().as_str().to_owned(), error)),
        }
    }

    Ok(failures)
}

/// Puts one file at `target`, replacing what is there.
fn replace_file(
    private: &Private,
    relative: &Path,
    target: &Path,
    content: ContentHash,
    source: &dyn ContentSource,
) -> Result<(), Error> {
    if target.exists() {
        if target.is_dir() {
            return Err(Error::Taken {
                path: target.display().to_string(),
            });
        }

        if digest_of(target)? == content {
            return Ok(());
        }

        fs::remove_file(target).map_err(Error::Io)?;
    }

    place_file(private, relative, target, content, source)
}

/// Puts one tree at `target`, writing every path the structure names over what is there.
fn replace_tree(
    private: &Private,
    relative: &Path,
    target: &Path,
    content: ContentHash,
    source: &dyn ContentSource,
) -> Result<(), Error> {
    let structure = tree::structure(source, content)?;

    // A restore writes the tree the lock names, so a copy taken out before it is the tree of some
    // other run: it goes rather than being written into.
    let staged = private.staging().join(relative);
    match fs::symlink_metadata(&staged) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(&staged).map_err(Error::Io)?,
        Ok(_) => fs::remove_file(&staged).map_err(Error::Io)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::Io(error)),
    }

    tree::replace(source, &structure, target)
}

/// Puts one file at `target`, taking it out of the private area when it is already there.
fn place_file(
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

/// Puts one tree at `target`, moving a staged copy back where there is one.
///
/// A staged copy is the tree as it was taken out, which is not necessarily the tree the lock names
/// now: a checkout that changed the tree leaves the path the same and the content different. What
/// the copy holds is compared with the structure, and rewritten where it differs. Paths the
/// structure does not name are left where they are, so a person's files inside the tree's
/// directory survive the round trip rather than being taken away with the tree.
fn place_tree(
    private: &Private,
    relative: &Path,
    target: &Path,
    content: ContentHash,
    source: &dyn ContentSource,
) -> Result<(), Error> {
    let structure = tree::structure(source, content)?;
    let staged = private.staging().join(relative);

    if staged.is_dir() {
        if occupied(target) {
            return Err(Error::Taken {
                path: target.display().to_string(),
            });
        }

        tree::move_directory(&staged, target)?;

        if tree::differences(target, &structure)?.is_empty() {
            return Ok(());
        }

        return tree::write(source, &structure, target);
    }

    if occupied(target) {
        if !target.is_dir() || !tree::differences(target, &structure)?.is_empty() {
            return Err(Error::Taken {
                path: target.display().to_string(),
            });
        }

        return Ok(());
    }

    if let Err(error) = tree::write(source, &structure, target) {
        // The directory was this run's own, so a half written one is taken away rather than left
        // for the next run to mistake for a place already holding something.
        let _ = fs::remove_dir_all(target);

        return Err(error);
    }

    Ok(())
}

/// Whether anything at all is at `path`.
fn occupied(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
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
            // WORKAROUND: a staging area and a working tree can be on different volumes, a project
            // spread over mounts, where a rename is not one operation. The copy is the fallback,
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
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    use rola_define_format::{ContentHash, Lock, LockPath, Material, MaterialKind};
    use rorolala_storage::Key;
    use rorolala_tree::{FileMode, TreeEntry, TreeStructure};

    use super::{Placed, Private, State, attach, detach, restore};
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

    /// A file material at `path`, holding `digest`.
    fn material(path: &str, digest: [u8; 32]) -> Material {
        Material::new(
            LockPath::new(path).unwrap(),
            ContentHash::from_digest(digest),
            MaterialKind::File,
        )
    }

    /// A tree material at `path`, holding `digest`.
    fn tree_material(path: &str, digest: [u8; 32]) -> Material {
        Material::new(
            LockPath::new(path).unwrap(),
            ContentHash::from_digest(digest),
            MaterialKind::Tree,
        )
    }

    /// A lock placing `paths` at one content each.
    fn lock(paths: &[(&str, [u8; 32])]) -> Lock {
        let materials = paths
            .iter()
            .map(|(path, digest)| material(path, *digest))
            .collect();

        Lock::new(vec![], materials).unwrap()
    }

    /// Puts `content` at `path` and brings the state in line with it.
    fn attach_text(working: &Path, path: &str, content: &[u8], state: &mut State) {
        let file = working.join(path.trim_start_matches('/'));
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, content).unwrap();
        let digest = digest_of(&file).unwrap();
        state.attached.insert(
            path.to_owned(),
            Placed::new(digest.to_token(), MaterialKind::File),
        );
    }

    /// A source that holds nothing.
    struct Nothing;

    impl ContentSource for Nothing {
        fn write(&self, content: ContentHash, _path: &Path) -> Result<(), Error> {
            Err(Error::Unavailable {
                content: content.to_token(),
            })
        }

        fn read(&self, content: ContentHash) -> Result<Vec<u8>, Error> {
            Err(Error::Unavailable {
                content: content.to_token(),
            })
        }
    }

    /// A source holding the contents it was given.
    #[derive(Default)]
    struct Blocks(BTreeMap<ContentHash, Vec<u8>>);

    impl Blocks {
        /// Holds `bytes` and answers with the content it is.
        fn hold(&mut self, bytes: &[u8]) -> ContentHash {
            let content = ContentHash::from_digest(*blake3::hash(bytes).as_bytes());
            self.0.insert(content, bytes.to_vec());

            content
        }
    }

    impl ContentSource for Blocks {
        fn write(&self, content: ContentHash, path: &Path) -> Result<(), Error> {
            let bytes = self.0.get(&content).ok_or_else(|| Error::Unavailable {
                content: content.to_token(),
            })?;

            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(Error::Io)?;
            }

            fs::write(path, bytes).map_err(Error::Io)
        }

        fn read(&self, content: ContentHash) -> Result<Vec<u8>, Error> {
            self.0
                .get(&content)
                .cloned()
                .ok_or_else(|| Error::Unavailable {
                    content: content.to_token(),
                })
        }
    }

    #[test]
    fn detaching_takes_materials_out_of_the_working_tree() {
        let dir = scratch("detach");
        let working = dir.join("work");
        let private = Private::of(&working);
        std::fs::create_dir_all(&working).unwrap();

        let mut state = State::default();
        attach_text(&working, "/Assets/hero.psd", b"hero", &mut state);

        detach(&private, &working, &mut state, &Nothing).unwrap();

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
            Placed::new(
                ContentHash::from_digest([1; 32]).to_token(),
                MaterialKind::File,
            ),
        );

        let error = detach(&private, &working, &mut state, &Nothing).unwrap_err();

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
            Placed::new(
                ContentHash::from_digest([1; 32]).to_token(),
                MaterialKind::File,
            ),
        );

        detach(&private, &working, &mut state, &Nothing).unwrap();

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
            Placed::new(digest_of(&file).unwrap().to_token(), MaterialKind::File),
        );
        detach(&private, &working, &mut state, &Nothing).unwrap();

        let store = LocalStore::new(dir.join("store"));
        let object = store.object_path(ContentHash::from_digest(digest));
        std::fs::create_dir_all(object.parent().unwrap()).unwrap();
        std::fs::write(&object, b"hero").unwrap();

        let lock = lock(&[("/Assets/hero.psd", digest)]);
        let failures = attach(&private, &working, &lock, &store, &mut state).unwrap();

        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(std::fs::read(&file).unwrap(), b"hero");
        let recorded = state.attached.get("Assets/hero.psd").cloned();
        assert_eq!(
            recorded,
            Some(Placed::new(
                digest_of(&file).unwrap().to_token(),
                MaterialKind::File
            ))
        );

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
        detach(&private, &working, &mut state, &Nothing).unwrap();

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
            Placed::new(
                ContentHash::from_digest([4; 32]).to_token(),
                MaterialKind::File,
            ),
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
        struct Counting(std::cell::Cell<usize>);

        impl ContentSource for Counting {
            fn write(&self, _content: ContentHash, _path: &Path) -> Result<(), Error> {
                self.0.set(self.0.get() + 1);

                Err(Error::Unavailable {
                    content: String::new(),
                })
            }

            fn read(&self, _content: ContentHash) -> Result<Vec<u8>, Error> {
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
        let source = Counting::default();
        let failures = attach(&private, &working, &lock, &source, &mut state).unwrap();

        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(source.0.get(), 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_state_written_before_kinds_were_recorded_reads_as_a_file() {
        let dir = scratch("old-state");
        let path = dir.join("state.toml");
        let content = ContentHash::from_digest([5; 32]).to_token();
        fs::write(
            &path,
            format!("[attached]\n\"Assets/hero.psd\" = \"{content}\"\n"),
        )
        .unwrap();

        let read = State::read(&path).unwrap();
        let placed = read.attached.get("Assets/hero.psd").unwrap();

        assert_eq!(placed.kind().unwrap(), MaterialKind::File);
        assert_eq!(placed.content().unwrap(), ContentHash::from_digest([5; 32]));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A structure holding `a` at `a.png` and `b` at `sub/b.psd`, and the content it is stored as.
    fn structure(blocks: &mut Blocks, a: &[u8], b: &[u8]) -> (TreeStructure, ContentHash) {
        let first = blocks.hold(a);
        let second = blocks.hold(b);
        let structure = TreeStructure::build(
            None,
            [
                TreeEntry::file("a.png", FileMode::Regular, Key::new(*first.digest())),
                TreeEntry::directory("sub"),
                TreeEntry::file("sub/b.psd", FileMode::Regular, Key::new(*second.digest())),
            ],
        )
        .unwrap();
        let content = blocks.hold(&structure.encode());

        (structure, content)
    }

    /// A lock placing one tree at `/Assets/Textures`, holding `content`.
    fn tree_lock(content: ContentHash) -> Lock {
        Lock::new(
            vec![],
            vec![tree_material("/Assets/Textures", *content.digest())],
        )
        .unwrap()
    }

    #[test]
    fn a_tree_is_placed_as_the_directory_its_structure_names() {
        let dir = scratch("tree-attach");
        let working = dir.join("work");
        let private = Private::of(&working);
        fs::create_dir_all(&working).unwrap();

        let mut blocks = Blocks::default();
        let (_, content) = structure(&mut blocks, b"a", b"b");
        let lock = tree_lock(content);

        let mut state = State::default();
        let failures = attach(&private, &working, &lock, &blocks, &mut state).unwrap();

        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(
            fs::read(working.join("Assets/Textures/a.png")).unwrap(),
            b"a"
        );
        assert_eq!(
            fs::read(working.join("Assets/Textures/sub/b.psd")).unwrap(),
            b"b"
        );
        assert!(state.attached.contains_key("Assets/Textures"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tree_already_placed_is_left_alone() {
        let dir = scratch("tree-again");
        let working = dir.join("work");
        let private = Private::of(&working);
        fs::create_dir_all(&working).unwrap();

        let mut blocks = Blocks::default();
        let (_, content) = structure(&mut blocks, b"a", b"b");
        let lock = tree_lock(content);

        let mut state = State::default();
        attach(&private, &working, &lock, &blocks, &mut state).unwrap();
        let failures = attach(&private, &working, &lock, &blocks, &mut state).unwrap();

        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(
            fs::read(working.join("Assets/Textures/a.png")).unwrap(),
            b"a"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tree_whose_place_holds_another_tree_is_refused() {
        let dir = scratch("tree-taken");
        let working = dir.join("work");
        let private = Private::of(&working);
        fs::create_dir_all(working.join("Assets/Textures")).unwrap();
        fs::write(working.join("Assets/Textures/a.png"), b"someone else").unwrap();

        let mut blocks = Blocks::default();
        let (_, content) = structure(&mut blocks, b"a", b"b");
        let lock = tree_lock(content);

        let mut state = State::default();
        let failures = attach(&private, &working, &lock, &blocks, &mut state).unwrap();

        assert_eq!(failures.len(), 1);
        assert!(
            matches!(failures[0].1, Error::Taken { .. }),
            "{:?}",
            failures[0].1
        );
        assert_eq!(
            fs::read(working.join("Assets/Textures/a.png")).unwrap(),
            b"someone else"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detaching_takes_the_whole_tree_out() {
        let dir = scratch("tree-detach");
        let working = dir.join("work");
        let private = Private::of(&working);
        fs::create_dir_all(&working).unwrap();

        let mut blocks = Blocks::default();
        let (_, content) = structure(&mut blocks, b"a", b"b");
        let lock = tree_lock(content);

        let mut state = State::default();
        attach(&private, &working, &lock, &blocks, &mut state).unwrap();
        detach(&private, &working, &mut state, &blocks).unwrap();

        assert!(!working.join("Assets/Textures").exists());
        assert!(private.staging().join("Assets/Textures").is_dir());
        assert!(private.staging().join("Assets/Textures/a.png").is_file());
        assert!(state.attached.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tree_a_person_changed_is_not_taken_out() {
        let dir = scratch("tree-changed");
        let working = dir.join("work");
        let private = Private::of(&working);
        fs::create_dir_all(&working).unwrap();

        let mut blocks = Blocks::default();
        let (_, content) = structure(&mut blocks, b"a", b"b");
        let lock = tree_lock(content);

        let mut state = State::default();
        attach(&private, &working, &lock, &blocks, &mut state).unwrap();
        fs::write(working.join("Assets/Textures/a.png"), b"edited").unwrap();

        let error = detach(&private, &working, &mut state, &blocks).unwrap_err();

        assert!(
            matches!(&error, Error::TreeContent { at, .. } if at == "a.png"),
            "{error}"
        );
        assert!(working.join("Assets/Textures").is_dir());
        assert!(!private.staging().join("Assets/Textures").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tree_missing_a_file_is_not_taken_out() {
        let dir = scratch("tree-missing");
        let working = dir.join("work");
        let private = Private::of(&working);
        fs::create_dir_all(&working).unwrap();

        let mut blocks = Blocks::default();
        let (_, content) = structure(&mut blocks, b"a", b"b");
        let lock = tree_lock(content);

        let mut state = State::default();
        attach(&private, &working, &lock, &blocks, &mut state).unwrap();
        fs::remove_file(working.join("Assets/Textures/sub/b.psd")).unwrap();

        let error = detach(&private, &working, &mut state, &blocks).unwrap_err();

        assert!(
            matches!(&error, Error::TreeContent { at, .. } if at == "sub/b.psd"),
            "{error}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn attaching_a_tree_after_a_detach_moves_the_staged_copy_back() {
        let dir = scratch("tree-round-trip");
        let working = dir.join("work");
        let private = Private::of(&working);
        fs::create_dir_all(&working).unwrap();

        let mut blocks = Blocks::default();
        let (_, content) = structure(&mut blocks, b"a", b"b");
        let lock = tree_lock(content);

        let mut state = State::default();
        attach(&private, &working, &lock, &blocks, &mut state).unwrap();
        detach(&private, &working, &mut state, &blocks).unwrap();
        let failures = attach(&private, &working, &lock, &blocks, &mut state).unwrap();

        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(
            fs::read(working.join("Assets/Textures/a.png")).unwrap(),
            b"a"
        );
        assert!(!private.staging().join("Assets/Textures").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_staged_tree_of_another_version_is_written_over_and_an_extra_file_survives() {
        let dir = scratch("tree-other-version");
        let working = dir.join("work");
        let private = Private::of(&working);
        fs::create_dir_all(&working).unwrap();

        let mut blocks = Blocks::default();
        let (_, first) = structure(&mut blocks, b"first-a", b"first-b");
        let mut state = State::default();
        attach(&private, &working, &tree_lock(first), &blocks, &mut state).unwrap();
        detach(&private, &working, &mut state, &blocks).unwrap();

        fs::write(
            private.staging().join("Assets/Textures/notes.txt"),
            b"notes",
        )
        .unwrap();

        let (_, second) = structure(&mut blocks, b"second-a", b"first-b");
        let failures = attach(&private, &working, &tree_lock(second), &blocks, &mut state).unwrap();

        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(
            fs::read(working.join("Assets/Textures/a.png")).unwrap(),
            b"second-a"
        );
        assert_eq!(
            fs::read(working.join("Assets/Textures/notes.txt")).unwrap(),
            b"notes"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn restoring_a_tree_writes_the_structure_over_what_is_there() {
        let dir = scratch("tree-restore");
        let working = dir.join("work");
        let private = Private::of(&working);
        fs::create_dir_all(&working).unwrap();

        let mut blocks = Blocks::default();
        let (_, content) = structure(&mut blocks, b"a", b"b");
        let lock = tree_lock(content);

        let mut state = State::default();
        attach(&private, &working, &lock, &blocks, &mut state).unwrap();
        fs::write(working.join("Assets/Textures/a.png"), b"edited").unwrap();

        let failures = restore(&private, &working, &lock, &[], &blocks, &mut state).unwrap();

        assert!(failures.is_empty(), "{failures:?}");
        assert_eq!(
            fs::read(working.join("Assets/Textures/a.png")).unwrap(),
            b"a"
        );
        assert!(state.attached.contains_key("Assets/Textures"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn restoring_a_tree_replaces_a_file_at_its_path() {
        let dir = scratch("tree-restore-file");
        let working = dir.join("work");
        let private = Private::of(&working);
        fs::create_dir_all(working.join("Assets")).unwrap();
        fs::write(working.join("Assets/Textures"), b"a file").unwrap();

        let mut blocks = Blocks::default();
        let (_, content) = structure(&mut blocks, b"a", b"b");
        let lock = tree_lock(content);

        let mut state = State::default();
        let failures = restore(&private, &working, &lock, &[], &blocks, &mut state).unwrap();

        assert!(failures.is_empty(), "{failures:?}");
        assert!(working.join("Assets/Textures").is_dir());
        assert_eq!(
            fs::read(working.join("Assets/Textures/a.png")).unwrap(),
            b"a"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
