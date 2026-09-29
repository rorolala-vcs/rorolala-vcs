//! A set of named Layouts, and the one that is checked out.
//!
//! A Workspace holds many Layouts and works in one of them at a time; a Vault holds one and has
//! no use for a name of its own on disk. This is the many: a directory of a Layout each, named by
//! the Layout's name, beside a file naming the one that is checked out.
//!
//! A name is a directory name, so it is held to being one: kebab-case, which is one word of
//! letters and digits with the words joined by `-`. That is what keeps a name from being a path —
//! no separator, no `.`, no `..` — and what keeps one Layout from being found under another.
//!
//! Beside each Layout's own files sits a `TRACK` file, naming the Vault upstream the Layout
//! tracks. It is a name the Workspace has bound rather than an address, so what a Layout tracks is
//! read from the Workspace's configuration; here it is only kept.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use just_fmt::fmt_case_style::CaseFormatter;
use rorolala_utils_constants::LAYOUT_TRACK_FILE;

use crate::error::LayoutError;
use crate::layout::Layout;

/// A set of named Layouts, and the one that is checked out.
pub struct Layouts {
    /// Where the named Layouts sit, one directory each.
    dir: PathBuf,

    /// The file naming the Layout that is checked out.
    current_path: PathBuf,
}

impl Layouts {
    /// The Layouts kept under `dir`, with `current_path` naming the one that is checked out.
    ///
    /// Nothing is made or read here: a set that is not there yet is one that has no Layouts, and
    /// saying which is which is each answer's own business.
    #[must_use]
    pub fn open(dir: impl AsRef<Path>, current_path: impl AsRef<Path>) -> Self {
        Self {
            dir: dir.as_ref().to_path_buf(),
            current_path: current_path.as_ref().to_path_buf(),
        }
    }

    /// Where the named Layouts sit.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The file naming the Layout that is checked out.
    #[must_use]
    pub fn current_path(&self) -> &Path {
        &self.current_path
    }

    /// The names of the Layouts here, in name order.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::Io`] if the directory cannot be read.
    pub fn names(&self) -> Result<Vec<String>, LayoutError> {
        let mut names = Vec::new();

        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            // A set with no directory is a set with no Layouts, which is not an error: nothing has
            // been made here yet.
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(names),
            Err(error) => return Err(error.into()),
        };

        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                && let Some(name) = entry.file_name().to_str()
            {
                names.push(name.to_owned());
            }
        }

        names.sort_unstable();

        Ok(names)
    }

    /// Whether a Layout by this name is here.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.dir.join(name).is_dir()
    }

    /// The Layout named `name`, or nothing when no Layout is.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::Io`] if the Layout is there but cannot be read back.
    pub fn get(&self, name: &str) -> Result<Option<Layout>, LayoutError> {
        if !self.contains(name) {
            return Ok(None);
        }

        Ok(Some(Layout::open(self.dir.join(name))?))
    }

    /// The name of the Layout that is checked out, or nothing when none is.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::Io`] if the file naming it cannot be read.
    pub fn current(&self) -> Result<Option<String>, LayoutError> {
        match fs::read_to_string(&self.current_path) {
            Ok(text) => {
                let name = text.trim();

                Ok((!name.is_empty()).then(|| name.to_owned()))
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Makes `name` the Layout that is checked out.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::NotFound`] if there is no Layout by that name, and [`LayoutError::Io`]
    /// if the file naming it cannot be written.
    pub fn set_current(&self, name: &str) -> Result<(), LayoutError> {
        if !self.contains(name) {
            return Err(LayoutError::NotFound);
        }

        if let Some(parent) = self.current_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.current_path, format!("{name}\n"))?;

        Ok(())
    }

    /// Makes a Layout by this name, empty.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::Name`] if the name is not one a Layout may be given,
    /// [`LayoutError::AlreadyExists`] if there is one by that name, and [`LayoutError::Io`] if it
    /// cannot be made.
    pub fn create(&self, name: &str) -> Result<Layout, LayoutError> {
        checked(name)?;
        if self.contains(name) {
            return Err(LayoutError::AlreadyExists);
        }

        Layout::open(self.dir.join(name))
    }

    /// Copies the Layout `from` to a Layout named `to`, which is empty of tracking.
    ///
    /// What is copied is the Layout's own files; whatever the Layout it came from tracks is not,
    /// since a copy is a new place to work rather than a second name for the same one.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::NotFound`] if there is no Layout named `from`,
    /// [`LayoutError::Name`] if `to` is not one a Layout may be given, [`LayoutError::AlreadyExists`]
    /// if there is one named `to` already, and [`LayoutError::Io`] if the copy cannot be made.
    pub fn copy(&self, from: &str, to: &str) -> Result<Layout, LayoutError> {
        if !self.contains(from) {
            return Err(LayoutError::NotFound);
        }

        checked(to)?;
        if self.contains(to) {
            return Err(LayoutError::AlreadyExists);
        }

        copy_dir(&self.dir.join(from), &self.dir.join(to))?;

        Layout::open(self.dir.join(to))
    }

    /// Drops the Layout by this name, and everything it holds.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::NotFound`] if there is none by that name, and [`LayoutError::Io`] if
    /// it cannot be taken away.
    pub fn remove(&self, name: &str) -> Result<(), LayoutError> {
        if !self.contains(name) {
            return Err(LayoutError::NotFound);
        }

        fs::remove_dir_all(self.dir.join(name))?;

        Ok(())
    }

    /// The Vault the Layout by this name tracks, or nothing when it tracks none.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::NotFound`] if there is no Layout by that name, and [`LayoutError::Io`]
    /// if the file naming its Vault cannot be read.
    pub fn track(&self, name: &str) -> Result<Option<String>, LayoutError> {
        if !self.contains(name) {
            return Err(LayoutError::NotFound);
        }

        match fs::read_to_string(self.dir.join(name).join(LAYOUT_TRACK_FILE)) {
            Ok(text) => {
                let track = text.trim();

                Ok((!track.is_empty()).then(|| track.to_owned()))
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Makes the Layout by this name track the Vault by `track`, or track none when it is empty.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::NotFound`] if there is no Layout by that name, and [`LayoutError::Io`]
    /// if the file naming its Vault cannot be written.
    pub fn set_track(&self, name: &str, track: &str) -> Result<(), LayoutError> {
        if !self.contains(name) {
            return Err(LayoutError::NotFound);
        }

        let track = track.trim();
        if track.is_empty() {
            return self.un_track(name);
        }

        fs::write(
            self.dir.join(name).join(LAYOUT_TRACK_FILE),
            format!("{track}\n"),
        )?;

        Ok(())
    }

    /// Makes the Layout by this name track no Vault.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::NotFound`] if there is no Layout by that name, and [`LayoutError::Io`]
    /// if what it names its Vault in cannot be taken away.
    pub fn un_track(&self, name: &str) -> Result<(), LayoutError> {
        if !self.contains(name) {
            return Err(LayoutError::NotFound);
        }

        match fs::remove_file(self.dir.join(name).join(LAYOUT_TRACK_FILE)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

/// Refuses `name` unless it is one a Layout may be given: kebab-case, and not nothing.
fn checked(name: &str) -> Result<(), LayoutError> {
    // What makes a name kebab-case is that it is already what the formatter would make of it: a
    // word of letters and digits has no separator to be split on and comes back as itself, while a
    // path, a run of mixed case, or nothing at all does not.
    if name.is_empty() || CaseFormatter::from(name).to_kebab_case() != name {
        return Err(LayoutError::Name(name.to_owned()));
    }

    Ok(())
}

/// Copies the directory `from` to `to`, leaving whatever names a tracked Vault behind.
fn copy_dir(from: &Path, to: &Path) -> Result<(), LayoutError> {
    fs::create_dir_all(to)?;

    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_str() == Some(LAYOUT_TRACK_FILE) {
            continue;
        }

        let target = to.join(&name);
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use uuid::Uuid;

    use super::Layouts;
    use crate::data::MutableData;
    use crate::error::LayoutError;
    use crate::path::LayoutPath;

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-layouts-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// A set of Layouts in a directory of its own.
    fn layouts(label: &str) -> Layouts {
        let dir = scratch(label);

        Layouts::open(dir.join("layouts"), dir.join("LAYOUT"))
    }

    #[test]
    fn a_layout_is_made_named_and_dropped() {
        let set = layouts("make");

        let layout = set.create("main").unwrap();
        layout
            .create_entry(
                Uuid::from_u128(1),
                MutableData::new(None, [1; 32], "x".to_owned()),
            )
            .unwrap();

        assert!(set.contains("main"));
        assert_eq!(set.names().unwrap(), vec!["main".to_owned()]);

        // Making one by a name already taken is refused rather than replaced.
        assert!(matches!(
            set.create("main"),
            Err(LayoutError::AlreadyExists)
        ));

        // And what was made is a Layout that reads back what was written to it.
        let read = set.get("main").unwrap().unwrap();
        assert_eq!(
            read.entry(Uuid::from_u128(1)),
            Some(MutableData::new(None, [1; 32], "x".to_owned()))
        );

        set.remove("main").unwrap();
        assert!(!set.contains("main"));
        assert!(set.names().unwrap().is_empty());
        assert!(matches!(set.remove("main"), Err(LayoutError::NotFound)));
    }

    #[test]
    fn a_name_that_is_not_kebab_case_is_refused() {
        let set = layouts("names");

        for bad in ["", ".", "..", "a/b", "My Layout", "MyLayout", "a_b"] {
            assert!(
                matches!(set.create(bad), Err(LayoutError::Name(_))),
                "{bad}"
            );
        }

        assert!(set.create("my-layout-2").is_ok());
    }

    #[test]
    fn a_name_that_is_checked_out_is_the_one_named() {
        let set = layouts("current");

        assert_eq!(set.current().unwrap(), None);

        set.create("main").unwrap();
        set.set_current("main").unwrap();
        assert_eq!(set.current().unwrap(), Some("main".to_owned()));

        // Choosing a Layout that is not there is what a name with nothing behind it is.
        assert!(matches!(
            set.set_current("gone"),
            Err(LayoutError::NotFound)
        ));
    }

    #[test]
    fn what_a_layout_tracks_is_named_and_can_be_let_go_of() {
        let set = layouts("track");
        set.create("main").unwrap();

        assert_eq!(set.track("main").unwrap(), None);

        set.set_track("main", "the-vault").unwrap();
        assert_eq!(set.track("main").unwrap(), Some("the-vault".to_owned()));

        // Setting nothing is the same as letting go.
        set.set_track("main", "  ").unwrap();
        assert_eq!(set.track("main").unwrap(), None);

        set.set_track("main", "the-vault").unwrap();
        set.un_track("main").unwrap();
        assert_eq!(set.track("main").unwrap(), None);
    }

    #[test]
    fn a_copy_holds_what_the_layout_held_but_tracks_nothing() {
        let set = layouts("copy");

        let from = set.create("main").unwrap();
        from.create_entry(
            Uuid::from_u128(2),
            MutableData::new(None, [2; 32], "y".to_owned()),
        )
        .unwrap();
        from.create_path(&LayoutPath::new("a.txt").unwrap(), Uuid::from_u128(2))
            .unwrap();
        drop(from);
        set.set_track("main", "the-vault").unwrap();

        let to = set.copy("main", "side").unwrap();
        assert_eq!(
            to.entry(Uuid::from_u128(2)),
            Some(MutableData::new(None, [2; 32], "y".to_owned()))
        );
        assert_eq!(
            to.id_of(&LayoutPath::new("a.txt").unwrap()),
            Some(Uuid::from_u128(2))
        );

        // A copy is a new place to work, so it does not track what the Layout it came from did.
        assert_eq!(set.track("side").unwrap(), None);
        // The Layout it came from still tracks its own.
        assert_eq!(set.track("main").unwrap(), Some("the-vault".to_owned()));

        assert!(matches!(
            set.copy("main", "main"),
            Err(LayoutError::AlreadyExists)
        ));
        assert!(matches!(
            set.copy("gone", "other"),
            Err(LayoutError::NotFound)
        ));
    }
}
