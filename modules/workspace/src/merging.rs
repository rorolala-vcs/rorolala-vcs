//! The merge in progress: the variant files checked in to be joined into the files they sit beside.
//!
//! A file the Layout names may have changes made elsewhere that are to be joined into it without
//! either side giving up: content a Vault holds, brought into the tree as a **variant file** and
//! joined into the file when the work is recorded. What says which variant is waiting for which file
//! is [`MERGING`](Merging::path), a plain file under the Workspace's own data directory: while it is
//! there and holds a line, the Workspace is in a merge.
//!
//! A variant file is not a path the Layout names — the Layout is what the work is, and a variant
//! file is something waiting to become part of it — so the reading of the tree has to be told which
//! paths to leave out, which is what [`Merging::ignored`] answers. Its path is a convention rather
//! than a record: it lies beside the file it is for, named after it with the variant's short hash in
//! it, so a move of the target carries the variant file along without the record saying where it is.
//! A run that names a place of its own for it says so, as a path relative to the target's directory,
//! and that is the one thing the record holds besides the two hashes.
//!
//! Keeping the state here rather than in the Layout is deliberate: a merge is work in this tree and
//! nowhere else, so it travels with nothing and changing it changes no format. A record that cannot
//! be carried out — the target no longer named, the variant no longer held, the file gone or changed
//! underneath — is left where it is for a reading to report, rather than dropped quietly.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;

use rorolala_layout::{Layout, LayoutPath};
use rorolala_storage::{Blake3Hash, Key};
use uuid::Uuid;

use crate::DATA_DIR;

/// How much of a variant's hash names its file.
///
/// It is what a reader is shown everywhere else, so it is what a person writes down and recognises;
/// the whole hash is what the record holds, and the file is found by what the record says rather
/// than by its name being read back.
const SHORT: usize = 7;

/// The file, under a Workspace's data directory, that says a merge is in progress.
const MERGING_FILE: &str = "MERGING";

/// One variant waiting to be joined into one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    /// The `Uuid` of the file the variant is to be joined into.
    target: Uuid,
    /// The hash of the variant that was checked in.
    variant: Blake3Hash,
    /// Where the variant file lies, relative to the target's directory, when the run said.
    at: Option<PathBuf>,
}

impl Pending {
    /// A variant waiting for the file `target`, its file lying where it is said or beside the
    /// target when nothing is said.
    #[must_use]
    pub const fn new(target: Uuid, variant: Blake3Hash, at: Option<PathBuf>) -> Self {
        Self {
            target,
            variant,
            at,
        }
    }

    /// The `Uuid` of the file the variant is to be joined into.
    #[must_use]
    pub const fn target(&self) -> Uuid {
        self.target
    }

    /// The hash of the variant that was checked in.
    #[must_use]
    pub const fn variant(&self) -> &Blake3Hash {
        &self.variant
    }

    /// Where the variant file lies, relative to the target's directory, when the run said.
    #[must_use]
    pub fn at(&self) -> Option<&Path> {
        self.at.as_deref()
    }

    /// The line this record is written down as.
    fn line(&self) -> String {
        let mut line = format!("{} {}", self.target, Key::new(self.variant).hex());

        if let Some(at) = &self.at {
            line.push(' ');
            line.push_str(&at.to_string_lossy());
        }

        line
    }
}

/// The merge in progress, in the order the records were written.
#[derive(Debug, Default, Clone)]
pub struct Merging {
    /// The variants waiting, one line each.
    pending: Vec<Pending>,
}

impl Merging {
    /// The file the merge in progress is written in, under `root`.
    #[must_use]
    pub fn path(root: &Path) -> PathBuf {
        root.join(DATA_DIR).join(MERGING_FILE)
    }

    /// Reads the merge in progress under `root`.
    ///
    /// A file that is not there is no merge at all rather than a failure, since that is what a
    /// Workspace that has never merged looks like. A line that is not a record is left out rather
    /// than refused: what a build cannot read it does not act on, and what is left is still what the
    /// records it could read say.
    #[must_use]
    pub fn read(root: &Path) -> Self {
        let Ok(text) = fs::read_to_string(Self::path(root)) else {
            return Self::default();
        };

        Self {
            pending: text.lines().filter_map(Self::parse).collect(),
        }
    }

    /// Writes the merge in progress under `root`, or takes the file away when there is none.
    ///
    /// The file being there is what says a merge is in progress, so the last record to go takes it
    /// with it: a Workspace whose file was left behind empty would be read as merging and say
    /// nothing about what is.
    ///
    /// # Errors
    ///
    /// Returns what writing the file failed with.
    pub fn write(&self, root: &Path) -> io::Result<()> {
        let path = Self::path(root);

        if self.pending.is_empty() {
            return match fs::remove_file(&path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            };
        }

        if let Some(directory) = path.parent() {
            fs::create_dir_all(directory)?;
        }

        let mut text = String::new();
        for pending in &self.pending {
            text.push_str(&pending.line());
            text.push('\n');
        }

        fs::write(path, text)
    }

    /// The line as the record it writes down, when it is one.
    fn parse(line: &str) -> Option<Pending> {
        let (target, rest) = take_token(line)?;
        let (variant, rest) = take_token(rest)?;

        let target = Uuid::from_str(target).ok()?;
        let variant = *Key::from_str(variant).ok()?.digest();
        let at = rest.trim();

        Some(Pending::new(
            target,
            variant,
            (!at.is_empty()).then(|| PathBuf::from(at)),
        ))
    }

    /// Whether no merge is in progress at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Every variant waiting, in the order it was written.
    pub fn iter(&self) -> impl Iterator<Item = &Pending> {
        self.pending.iter()
    }

    /// The variant waiting for the file `target`, when one is.
    #[must_use]
    pub fn at(&self, target: Uuid) -> Option<&Pending> {
        self.pending.iter().find(|pending| pending.target == target)
    }

    /// Whether the variant `variant` was already checked in.
    #[must_use]
    pub fn holds_variant(&self, variant: &Blake3Hash) -> bool {
        self.pending
            .iter()
            .any(|pending| &pending.variant == variant)
    }

    /// Adds a variant waiting for a file.
    pub fn push(&mut self, pending: Pending) {
        self.pending.push(pending);
    }

    /// Takes the variant waiting for the file `target` away, answering whether one was.
    pub fn remove(&mut self, target: Uuid) -> bool {
        let before = self.pending.len();
        self.pending.retain(|pending| pending.target != target);
        self.pending.len() != before
    }

    /// The path the variant file for `pending` is at, when the Layout still names its target.
    #[must_use]
    pub fn variant_path(&self, layout: &Layout, pending: &Pending) -> Option<LayoutPath> {
        variant_path(
            &layout.path_of(pending.target)?,
            pending.at.as_deref(),
            &pending.variant,
        )
    }

    /// The variant waiting whose file lies at `path`, when one does.
    #[must_use]
    pub fn variant_of(&self, layout: &Layout, path: &LayoutPath) -> Option<&Pending> {
        self.pending.iter().find(|pending| {
            self.variant_path(layout, pending)
                .is_some_and(|variant| &variant == path)
        })
    }

    /// Says where the variant waiting for `target` lies now, answering whether one was waiting.
    ///
    /// It is what a run that moves a variant file itself writes down: the place is no longer left to
    /// the convention but said, as the way from the target's directory to the file.
    pub fn relocate(&mut self, target: Uuid, at: Option<PathBuf>) -> bool {
        let Some(pending) = self
            .pending
            .iter_mut()
            .find(|pending| pending.target == target)
        else {
            return false;
        };

        pending.at = at;

        true
    }

    /// The place `to` lies at, read from the directory of `target`.
    ///
    /// It is the way from the target's directory to the file, with `..` for each directory left
    /// behind, so that a target that later moves takes the file with it the same way it would take
    /// one lying beside it.
    #[must_use]
    pub fn place_of(target: &LayoutPath, to: &LayoutPath) -> PathBuf {
        let from = target
            .as_str()
            .rsplit_once('/')
            .map_or("", |(directory, _)| directory);
        let (to_directory, to_name) = to.as_str().rsplit_once('/').unwrap_or(("", to.as_str()));

        let from: Vec<&str> = from.split('/').filter(|part| !part.is_empty()).collect();
        let to_directory: Vec<&str> = to_directory
            .split('/')
            .filter(|part| !part.is_empty())
            .collect();
        let common = from
            .iter()
            .zip(&to_directory)
            .take_while(|(left, right)| left == right)
            .count();

        let mut place = PathBuf::new();
        for _ in common..from.len() {
            place.push("..");
        }
        for component in &to_directory[common..] {
            place.push(component);
        }
        place.push(to_name);

        place
    }

    /// The paths of every variant file waiting, for a reading of the tree to leave out.
    ///
    /// A record whose target the Layout no longer names names no path: what is left of it is nothing
    /// to leave out, and the record itself is what a reading reports.
    #[must_use]
    pub fn ignored(&self, layout: &Layout) -> BTreeSet<LayoutPath> {
        self.pending
            .iter()
            .filter_map(|pending| self.variant_path(layout, pending))
            .collect()
    }

    /// Moves the variant file waiting for `target` so that it lies where `to` puts it rather than
    /// `from`.
    ///
    /// A variant file lies beside its target under the convention, or under the relative place the
    /// run said, so a target that moves takes its variant file with it. Nothing about the record
    /// changes: it says where the file lies relative to the target, and the target is what moved.
    ///
    /// A record with nothing waiting at the old place, or one whose old and new places are the same,
    /// is nothing to move and answers as done.
    ///
    /// # Errors
    ///
    /// Returns what moving the variant file failed with.
    pub fn follow(
        &self,
        root: &Path,
        target: Uuid,
        from: &LayoutPath,
        to: &LayoutPath,
    ) -> io::Result<()> {
        let Some(pending) = self.at(target) else {
            return Ok(());
        };

        let (Some(before), Some(after)) = (
            variant_path(from, pending.at.as_deref(), &pending.variant),
            variant_path(to, pending.at.as_deref(), &pending.variant),
        ) else {
            return Ok(());
        };

        if before == after {
            return Ok(());
        }

        let source = root.join(before.to_path_buf());
        if !source.is_file() {
            return Ok(());
        }

        let destination = root.join(after.to_path_buf());
        if let Some(directory) = destination.parent() {
            fs::create_dir_all(directory)?;
        }

        fs::rename(source, destination)
    }
}

/// The path a variant file lies at: where the run said, or beside its target under the convention.
///
/// The convention is the target's name with the variant's short hash in it — `hero.1a2b3c4.psd`
/// where the target is `hero.psd`, and `README_1a2b3c4` where it has no suffix, since a suffix a
/// file did not have is not one to give it. The hash is there so the name says which variant it is
/// without anything being read, and the target's directory is there so both move together.
fn variant_path(
    target: &LayoutPath,
    at: Option<&Path>,
    variant: &Blake3Hash,
) -> Option<LayoutPath> {
    let (directory, name) = target
        .as_str()
        .rsplit_once('/')
        .unwrap_or(("", target.as_str()));

    // A place of the run's own is read from the target's directory, and one that reaches out of the
    // Workspace names nothing a Layout could hold.
    if at.is_some_and(Path::is_absolute) {
        return None;
    }

    let name = at.map_or_else(
        || {
            let derived = derived_name(name, variant);
            if directory.is_empty() {
                derived
            } else {
                format!("{directory}/{derived}")
            }
        },
        |at| {
            let mut said = String::from(directory);
            if !said.is_empty() {
                said.push('/');
            }
            said.push_str(&at.to_string_lossy());

            said
        },
    );

    LayoutPath::new(&name).ok()
}

/// The name a variant file takes beside a target named `target`.
fn derived_name(target: &str, variant: &Blake3Hash) -> String {
    let short = &Key::new(*variant).hex()[..SHORT];

    match target.rsplit_once('.') {
        // A leading dot is a name, not a suffix: `.gitignore` is a file called that rather than one
        // called nothing with the suffix `gitignore`.
        Some((stem, suffix)) if !stem.is_empty() && !suffix.is_empty() => {
            format!("{stem}.{short}.{suffix}")
        }
        _ => format!("{target}_{short}"),
    }
}

/// The first word of `text`, and what follows it.
fn take_token(text: &str) -> Option<(&str, &str)> {
    let text = text.trim_start();

    if text.is_empty() {
        return None;
    }

    Some(
        text.find(char::is_whitespace)
            .map_or((text, ""), |at| (&text[..at], &text[at..])),
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use rorolala_layout::LayoutPath;
    use uuid::Uuid;

    use super::{Merging, Pending, derived_name};

    /// A variant hash whose short form is `0102030`, so a name it makes is obvious.
    const VARIANT: [u8; 32] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
        0x1f, 0x20,
    ];

    #[test]
    fn a_suffix_is_kept_where_it_was_and_the_hash_goes_before_it() {
        assert_eq!(derived_name("hero.psd", &VARIANT), "hero.0102030.psd");
    }

    #[test]
    fn a_name_with_no_suffix_is_given_none() {
        assert_eq!(derived_name("README", &VARIANT), "README_0102030");
    }

    #[test]
    fn a_leading_dot_names_the_file_rather_than_a_suffix() {
        assert_eq!(derived_name(".gitignore", &VARIANT), ".gitignore_0102030");
    }

    #[test]
    fn only_the_last_dot_is_a_suffix() {
        assert_eq!(
            derived_name("hero.old.psd", &VARIANT),
            "hero.old.0102030.psd"
        );
    }

    #[test]
    fn a_record_is_written_and_read_back() {
        let target = Uuid::from_u128(7);
        let record = Pending::new(target, VARIANT, None);

        assert_eq!(Merging::parse(&record.line()), Some(record));
    }

    #[test]
    fn a_record_that_says_where_its_file_lies_reads_back_whole() {
        let target = Uuid::from_u128(7);
        let record = Pending::new(target, VARIANT, Some("sub/hero.0102030.psd".into()));

        assert_eq!(Merging::parse(&record.line()), Some(record));
    }

    #[test]
    fn a_place_with_spaces_is_kept_whole() {
        let target = Uuid::from_u128(7);
        let record = Pending::new(target, VARIANT, Some("a b/hero.psd".into()));

        assert_eq!(Merging::parse(&record.line()), Some(record));
    }

    #[test]
    fn a_line_that_names_no_record_is_refused() {
        assert!(Merging::parse("not a record at all").is_none());
        assert!(Merging::parse("").is_none());
    }

    /// The place `to` lies at, read from the file `target`.
    fn place(target: &str, to: &str) -> String {
        Merging::place_of(&path(target), &path(to))
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// A path a Layout would name.
    fn path(text: &str) -> LayoutPath {
        LayoutPath::new(text).expect("a path a Layout could name")
    }

    #[test]
    fn a_place_beside_the_target_is_the_name_alone() {
        assert_eq!(place("models/hero.psd", "models/other.psd"), "other.psd");
    }

    #[test]
    fn a_place_under_the_targets_directory_keeps_the_way_there() {
        assert_eq!(place("models/hero.psd", "models/sub/x.psd"), "sub/x.psd");
    }

    #[test]
    fn a_place_beside_the_targets_directory_climbs_out_of_it() {
        assert_eq!(place("models/hero.psd", "other/x.psd"), "../other/x.psd");
    }

    #[test]
    fn a_place_above_the_target_climbs_the_whole_way() {
        assert_eq!(place("models/sub/hero.psd", "models/x.psd"), "../x.psd");
    }

    #[test]
    fn a_target_at_the_root_reads_the_rest_of_the_way() {
        assert_eq!(place("hero.psd", "models/x.psd"), "models/x.psd");
    }

    #[test]
    fn relocating_a_record_says_where_its_file_now_lies() {
        let target = Uuid::from_u128(7);
        let mut merging = Merging::default();
        merging.push(Pending::new(target, VARIANT, None));

        assert!(merging.relocate(target, Some("moved.psd".into())));
        assert_eq!(
            merging.at(target).and_then(Pending::at),
            Some(Path::new("moved.psd"))
        );
        assert!(!merging.relocate(Uuid::from_u128(8), None));
    }
}
