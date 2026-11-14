//! The structure a tree version names: its paths, what each is, and the content each stands for.

use rorolala_storage::Key;
use std::collections::BTreeMap;

use crate::TreeError;
use unicode_normalization::UnicodeNormalization as _;

/// What a structure object starts with, so one this build did not write is refused rather than
/// misread.
const MAGIC: &[u8; 4] = b"RLTR";

/// The version of the structure format this build writes.
///
/// A version this build does not know is refused by name rather than left to a decoder that would
/// read the entries of another shape as this one's.
const VERSION: u8 = 1;

/// The bit of the flags byte that says a description reference follows.
const HAS_DESCRIPTION: u8 = 1;

/// What one entry of a tree is.
///
/// The numbers are the ones the structure is written down by, and they are this crate's own: a tree
/// entry's file, directory and link are not the file and tree a Layout entry names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum EntryKind {
    /// A file, whose content is stored under a key.
    File = 1,
    /// A directory, which may hold nothing.
    Directory = 2,
    /// A symbolic link, whose target is written down rather than followed.
    Symlink = 3,
}

impl EntryKind {
    /// The number this kind is written down by.
    #[must_use]
    pub const fn id(self) -> u8 {
        self as u8
    }

    /// The kind written down by `number`, if this build knows one.
    #[must_use]
    pub const fn from_id(number: u8) -> Option<Self> {
        match number {
            1 => Some(Self::File),
            2 => Some(Self::Directory),
            3 => Some(Self::Symlink),
            _ => None,
        }
    }
}

/// Whether a file is executable.
///
/// It is the whole of what the format says about a file's permissions: group and other bits are not
/// part of the work, and time is not content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum FileMode {
    /// An ordinary file.
    Regular = 0,
    /// A file whose owner may execute it.
    Executable = 1,
}

impl FileMode {
    /// The number this mode is written down by.
    #[must_use]
    pub const fn id(self) -> u8 {
        self as u8
    }

    /// The mode written down by `number`, if this build knows one.
    #[must_use]
    pub const fn from_id(number: u8) -> Option<Self> {
        match number {
            0 => Some(Self::Regular),
            1 => Some(Self::Executable),
            _ => None,
        }
    }
}

/// One path of a tree, and what it is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TreeEntry {
    /// The path, tree-root relative, `/` separated.
    path: String,
    /// What the path is.
    kind: EntryKind,
    /// Whether a file is executable. Only a file has one.
    mode: Option<FileMode>,
    /// The content a file stands for. Only a file has one.
    storage: Option<Key>,
    /// What a symbolic link points at. Only a link has one.
    target: Option<String>,
}

impl TreeEntry {
    /// A file at `path`, holding the content stored under `storage`.
    #[must_use]
    pub fn file(path: impl Into<String>, mode: FileMode, storage: Key) -> Self {
        Self {
            path: path.into(),
            kind: EntryKind::File,
            mode: Some(mode),
            storage: Some(storage),
            target: None,
        }
    }

    /// A directory at `path`, which may hold nothing.
    #[must_use]
    pub fn directory(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            kind: EntryKind::Directory,
            mode: None,
            storage: None,
            target: None,
        }
    }

    /// A symbolic link at `path`, pointing at `target`.
    #[must_use]
    pub fn symlink(path: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            kind: EntryKind::Symlink,
            mode: None,
            storage: None,
            target: Some(target.into()),
        }
    }

    /// The path, tree-root relative.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// This entry at another path.
    #[must_use]
    pub fn rebased(&self, path: &str) -> Self {
        let mut entry = self.clone();
        path.clone_into(&mut entry.path);

        entry
    }

    /// What the path is.
    #[must_use]
    pub const fn kind(&self) -> EntryKind {
        self.kind
    }

    /// Whether a file is executable, when this is a file.
    #[must_use]
    pub const fn mode(&self) -> Option<FileMode> {
        self.mode
    }

    /// The content a file stands for, when this is a file.
    #[must_use]
    pub const fn storage(&self) -> Option<&Key> {
        self.storage.as_ref()
    }

    /// What a symbolic link points at, when this is a link.
    #[must_use]
    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }
}

/// What a tree version holds.
///
/// The entries are canonical: ordered by path, every file's and every link's directories present as
/// directory entries, and no path named twice. A structure is built through [`build`](Self::build),
/// which makes it canonical, or read back through [`decode`](Self::decode), which refuses one that
/// is not.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TreeStructure {
    /// The key of the description object, when the version has one.
    description: Option<Key>,
    /// The entries, ordered by path.
    entries: Vec<TreeEntry>,
}

impl TreeStructure {
    /// The structure of no entries at all.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            description: None,
            entries: Vec::new(),
        }
    }

    /// The structure `entries` make, with `description`, made canonical.
    ///
    /// Every path is normalized, every directory a file or link needs is added where it is not
    /// named, the entries are ordered by path, and a path named twice or a path with a file for a
    /// parent is refused.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Path`] for a path a tree may not name, [`TreeError::Duplicate`] for a
    /// path named twice, and [`TreeError::Conflict`] for a path whose parent is a file or a link.
    pub fn build(
        description: Option<Key>,
        entries: impl IntoIterator<Item = TreeEntry>,
    ) -> Result<Self, TreeError> {
        let mut map: BTreeMap<String, TreeEntry> = BTreeMap::new();

        for mut entry in entries {
            let path = normalize_path(&entry.path)?;
            path.clone_into(&mut entry.path);
            validate_entry(&entry)?;

            if map.contains_key(&path) {
                return Err(TreeError::Duplicate(path));
            }

            map.insert(path, entry);
        }

        // Every directory a file or a link names must be an entry of its own, so a structure that
        // was built from paths alone still says what it holds in full, empty directories included.
        let paths: Vec<String> = map.keys().cloned().collect();
        for path in &paths {
            let mut at = path.as_str();

            while let Some((parent, _)) = at.rsplit_once('/') {
                if parent.is_empty() {
                    break;
                }

                match map.get(parent) {
                    Some(entry) if entry.kind == EntryKind::Directory => {}
                    Some(_) => return Err(TreeError::Conflict(parent.to_owned())),
                    None => {
                        map.insert(parent.to_owned(), TreeEntry::directory(parent));
                    }
                }

                at = parent;
            }
        }

        Ok(Self {
            description,
            entries: map.into_values().collect(),
        })
    }

    /// The key of the description object, when the version has one.
    #[must_use]
    pub const fn description(&self) -> Option<&Key> {
        self.description.as_ref()
    }

    /// This structure with description as the key of its description object.
    ///
    /// A staged structure carries no description: the text is staged in the tree's own file and only
    /// becomes part of the version when it is committed. It is the commit that puts the key here.
    #[must_use]
    pub const fn with_description(mut self, description: Option<Key>) -> Self {
        self.description = description;

        self
    }

    /// The content keys this structure names, the description's first.
    ///
    /// A version's structure object names the keys its files hold and, when it has one, the key of
    /// its description. Transferring a tree's content is transferring these: what the structure
    /// itself is carried as is the version's storage key, and what the entries hold is this list.
    #[must_use]
    pub fn content_keys(&self) -> Vec<Key> {
        let mut keys: Vec<Key> = Vec::new();
        if let Some(description) = &self.description {
            keys.push(*description);
        }

        for entry in &self.entries {
            if let Some(storage) = entry.storage()
                && !keys.contains(storage)
            {
                keys.push(*storage);
            }
        }

        keys
    }

    /// The entries under `prefix`, with paths rebased on it.
    ///
    /// A conflict on a directory puts the other side beside the work as a directory of its own, and
    /// what is written into it is this: the entries the directory held, with their paths taken
    /// relative to it. The directory's own entry is not part of it; it is the directory itself.
    #[must_use]
    pub fn subtree(&self, prefix: &str) -> Self {
        let prefix = prefix.trim_end_matches('/');
        let mut entries: Vec<TreeEntry> = Vec::new();

        for entry in &self.entries {
            let Some(rest) = entry.path().strip_prefix(prefix) else {
                continue;
            };
            let Some(relative) = rest.strip_prefix('/') else {
                continue;
            };
            if relative.is_empty() {
                continue;
            }

            entries.push(entry.rebased(relative));
        }

        Self::build(None, entries).unwrap_or_else(|_| Self::empty())
    }

    /// The entries, ordered by path.
    #[must_use]
    pub fn entries(&self) -> &[TreeEntry] {
        &self.entries
    }

    /// How many entries the structure holds.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the structure holds nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entry named `path`, if the structure holds one.
    #[must_use]
    pub fn entry(&self, path: &str) -> Option<&TreeEntry> {
        self.entries
            .binary_search_by(|entry| entry.path.as_str().cmp(path))
            .ok()
            .map(|at| &self.entries[at])
    }

    /// The structure written down, as [`decode`](Self::decode) reads it.
    ///
    /// The bytes are canonical because the structure is: the same files make the same bytes and so
    /// the same [`hash`](Self::hash), whatever order they were given in.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.push(VERSION);

        let flags = if self.description.is_some() {
            HAS_DESCRIPTION
        } else {
            0
        };
        bytes.push(flags);

        if let Some(description) = &self.description {
            bytes.extend_from_slice(description.digest());
        }

        let count = u64::try_from(self.entries.len()).unwrap_or(u64::MAX);
        bytes.extend_from_slice(&count.to_be_bytes());

        for entry in &self.entries {
            put_entry(&mut bytes, entry);
        }

        bytes
    }

    /// The key this structure is stored under: the BLAKE3 hash of its bytes.
    #[must_use]
    pub fn hash(&self) -> Key {
        Key::new(*blake3::hash(&self.encode()).as_bytes())
    }

    /// Whether `bytes` begin a structure this build writes.
    ///
    /// A caller that holds only the opening of a content, the first chunk of one kept cut, uses this
    /// to tell a tree's structure from a file's content before it reads the whole of it. What is
    /// read is the magic and the format version, not the entries: a content that begins one is meant
    /// as a structure, and [`decode`](Self::decode) is what says whether it holds together.
    #[must_use]
    pub fn begins(bytes: &[u8]) -> bool {
        bytes.starts_with(MAGIC) && bytes.get(MAGIC.len()) == Some(&VERSION)
    }

    /// The structure `bytes` hold, which [`encode`](Self::encode) is what made.
    ///
    /// # Errors
    ///
    /// Returns [`TreeError::Malformed`] when the bytes are not a structure this build wrote, or are
    /// not canonical, and the path and entry failures [`build`](Self::build) gives for a path a tree
    /// may not name.
    pub fn decode(bytes: &[u8]) -> Result<Self, TreeError> {
        let mut rest = bytes;

        let (magic, after) = rest
            .split_at_checked(MAGIC.len())
            .ok_or(TreeError::Malformed)?;
        if magic != MAGIC {
            return Err(TreeError::Malformed);
        }
        rest = after;

        let (&version, after) = rest.split_first().ok_or(TreeError::Malformed)?;
        if version != VERSION {
            return Err(TreeError::Malformed);
        }
        rest = after;

        let (&flags, after) = rest.split_first().ok_or(TreeError::Malformed)?;
        if flags & !HAS_DESCRIPTION != 0 {
            return Err(TreeError::Malformed);
        }
        rest = after;

        let description = if flags & HAS_DESCRIPTION != 0 {
            let (digest, after) = rest.split_at_checked(32).ok_or(TreeError::Malformed)?;
            let digest: [u8; 32] = digest.try_into().map_err(|_| TreeError::Malformed)?;
            rest = after;

            Some(Key::new(digest))
        } else {
            None
        };

        let (count, after) = rest.split_at_checked(8).ok_or(TreeError::Malformed)?;
        let count = usize::try_from(u64::from_be_bytes(count.try_into().unwrap_or_default()))
            .map_err(|_| TreeError::Malformed)?;
        rest = after;

        let mut entries = Vec::with_capacity(count.min(1024));
        for _ in 0..count {
            let (entry, after) = take_entry(rest)?;
            entries.push(entry);
            rest = after;
        }

        if !rest.is_empty() {
            return Err(TreeError::Malformed);
        }

        // A structure read back must already be canonical. What a build would add is a path this
        // reading will not: a file whose directory is not named is a structure this build did not
        // write, and reading it as canonical would make two byte strings one tree.
        validate_canonical(&entries)?;

        Ok(Self {
            description,
            entries,
        })
    }
}

/// The path `text` names, normalized, or a refusal.
fn normalize_path(text: &str) -> Result<String, TreeError> {
    // A tree path is relative to the tree's root. A leading separator is a path rooted somewhere
    // else on every platform, so it is not one a tree says.
    if text.starts_with(['/', '\\']) {
        return Err(TreeError::Path(text.to_owned()));
    }

    let mut parts: Vec<&str> = Vec::new();

    for part in text.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return Err(TreeError::Path(text.to_owned())),
            other => parts.push(other),
        }
    }

    if parts.is_empty() {
        return Err(TreeError::Path(text.to_owned()));
    }

    let mut normalized = String::new();
    for (at, part) in parts.iter().enumerate() {
        // A data or control directory is not work, and a conflict copy is what a merge left behind.
        if *part == ".rola" || *part == ".rola-tree" {
            return Err(TreeError::Path(text.to_owned()));
        }
        if part.contains("~theirs") {
            return Err(TreeError::Path(text.to_owned()));
        }
        // A colon is a drive or a stream rather than a name in the path, and a name holding one is
        // not one this build can lay down on every platform it runs on.
        if part.contains(':') {
            return Err(TreeError::Path(text.to_owned()));
        }
        if at > 0 {
            normalized.push('/');
        }
        normalized.extend(part.nfc());
    }

    Ok(normalized)
}

/// Fails unless `entry` is what a kind of entry is made of.
fn validate_entry(entry: &TreeEntry) -> Result<(), TreeError> {
    match entry.kind {
        EntryKind::File => {
            if entry.storage.is_none() || entry.mode.is_none() || entry.target.is_some() {
                return Err(TreeError::Malformed);
            }
        }
        EntryKind::Directory => {
            if entry.storage.is_some() || entry.mode.is_some() || entry.target.is_some() {
                return Err(TreeError::Malformed);
            }
        }
        EntryKind::Symlink => {
            let target = entry.target.as_deref();
            if target.is_none_or(str::is_empty) || entry.storage.is_some() || entry.mode.is_some() {
                return Err(TreeError::Malformed);
            }
        }
    }

    Ok(())
}

/// Fails unless `entries` are what a structure read back must be: canonical.
fn validate_canonical(entries: &[TreeEntry]) -> Result<(), TreeError> {
    let mut previous: Option<&str> = None;
    let mut by_path: BTreeMap<&str, &TreeEntry> = BTreeMap::new();

    for entry in entries {
        let path = normalize_path(&entry.path)?;
        if path != entry.path {
            return Err(TreeError::Malformed);
        }

        validate_entry(entry)?;

        if previous.is_some_and(|before| before >= entry.path.as_str()) {
            return Err(TreeError::Malformed);
        }
        previous = Some(&entry.path);
        by_path.insert(&entry.path, entry);
    }

    // Every directory a path is under is an entry of this structure, and it is a directory. The
    // entries are in path order, so the map is read once and every parent is a lookup.
    for entry in entries {
        let mut at = entry.path.as_str();
        while let Some((parent, _)) = at.rsplit_once('/') {
            if parent.is_empty() {
                break;
            }

            match by_path.get(parent) {
                Some(held) if held.kind == EntryKind::Directory => {}
                _ => return Err(TreeError::Malformed),
            }

            at = parent;
        }
    }

    Ok(())
}

/// Writes `entry` as the structure writes one.
fn put_entry(out: &mut Vec<u8>, entry: &TreeEntry) {
    out.push(entry.kind.id());
    put_text(out, &entry.path);

    match entry.kind {
        EntryKind::File => {
            out.push(entry.mode.unwrap_or(FileMode::Regular).id());
            if let Some(storage) = &entry.storage {
                out.extend_from_slice(storage.digest());
            }
        }
        EntryKind::Directory => {}
        EntryKind::Symlink => put_text(out, entry.target.as_deref().unwrap_or_default()),
    }
}

/// Writes a string: how long it is, then its UTF-8 bytes.
fn put_text(out: &mut Vec<u8>, text: &str) {
    let bytes = text.as_bytes();
    let length = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&length.to_be_bytes());

    // The bytes past what the length can say are not part of the string this format writes, which
    // only happens for a name longer than any path a Layout may hold anyway.
    let keep = bytes.len().min(usize::try_from(length).unwrap_or(0));
    out.extend_from_slice(&bytes[..keep]);
}

/// Reads one entry.
fn take_entry(bytes: &[u8]) -> Result<(TreeEntry, &[u8]), TreeError> {
    let (&kind, rest) = bytes.split_first().ok_or(TreeError::Malformed)?;
    let kind = EntryKind::from_id(kind).ok_or(TreeError::Malformed)?;

    let (path, rest) = take_text(rest)?;
    let (entry, rest) = match kind {
        EntryKind::File => {
            let (&mode, rest) = rest.split_first().ok_or(TreeError::Malformed)?;
            let mode = FileMode::from_id(mode).ok_or(TreeError::Malformed)?;
            let (storage, rest) = rest.split_at_checked(32).ok_or(TreeError::Malformed)?;
            let storage: [u8; 32] = storage.try_into().map_err(|_| TreeError::Malformed)?;

            (
                TreeEntry {
                    path,
                    kind,
                    mode: Some(mode),
                    storage: Some(Key::new(storage)),
                    target: None,
                },
                rest,
            )
        }
        EntryKind::Directory => (
            TreeEntry {
                path,
                kind,
                mode: None,
                storage: None,
                target: None,
            },
            rest,
        ),
        EntryKind::Symlink => {
            let (target, rest) = take_text(rest)?;

            (
                TreeEntry {
                    path,
                    kind,
                    mode: None,
                    storage: None,
                    target: Some(target),
                },
                rest,
            )
        }
    };

    Ok((entry, rest))
}

/// Reads a length and the UTF-8 bytes after it.
fn take_text(bytes: &[u8]) -> Result<(String, &[u8]), TreeError> {
    let (head, rest) = bytes.split_at_checked(4).ok_or(TreeError::Malformed)?;
    let length = usize::try_from(u32::from_be_bytes(head.try_into().unwrap_or_default()))
        .map_err(|_| TreeError::Malformed)?;

    let (text, rest) = rest.split_at_checked(length).ok_or(TreeError::Malformed)?;
    let text = std::str::from_utf8(text)
        .map_err(|_| TreeError::Malformed)?
        .to_owned();

    Ok((text, rest))
}

#[cfg(test)]
mod tests {
    use rorolala_storage::Key;

    use super::{
        EntryKind, FileMode, HAS_DESCRIPTION, MAGIC, TreeEntry, TreeStructure, VERSION, put_entry,
        validate_entry,
    };
    use crate::TreeError;

    /// A key standing for content a test does not need.
    fn key(seed: u8) -> Key {
        Key::new([seed; 32])
    }

    /// A structure with one of everything, a description included.
    fn full() -> TreeStructure {
        TreeStructure::build(
            Some(key(9)),
            [
                TreeEntry::file("textures/a.psd", FileMode::Regular, key(1)),
                TreeEntry::file("build.sh", FileMode::Executable, key(2)),
                TreeEntry::directory("renders"),
                TreeEntry::symlink("latest.psd", "textures/a.psd"),
                TreeEntry::directory("textures"),
            ],
        )
        .unwrap()
    }

    /// The bytes of `entries` as a structure this build writes, without making them canonical.
    fn raw(entries: &[TreeEntry], description: Option<Key>) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.push(VERSION);
        bytes.push(if description.is_some() {
            HAS_DESCRIPTION
        } else {
            0
        });
        if let Some(description) = description {
            bytes.extend_from_slice(description.digest());
        }
        bytes.extend_from_slice(&u64::try_from(entries.len()).unwrap().to_be_bytes());
        for entry in entries {
            put_entry(&mut bytes, entry);
        }

        bytes
    }

    #[test]
    fn a_structure_round_trips_with_every_kind_of_entry() {
        let structure = full();
        let read = TreeStructure::decode(&structure.encode()).unwrap();

        assert_eq!(read, structure);
        assert_eq!(read.description(), Some(&key(9)));

        let entries = read.entries();
        let paths: Vec<&str> = entries.iter().map(TreeEntry::path).collect();
        assert_eq!(
            paths,
            [
                "build.sh",
                "latest.psd",
                "renders",
                "textures",
                "textures/a.psd"
            ]
        );
    }

    #[test]
    fn the_directories_a_path_needs_are_entries_of_their_own() {
        let structure = TreeStructure::build(
            None,
            [TreeEntry::file("a/b/c.psd", FileMode::Regular, key(1))],
        )
        .unwrap();

        let paths: Vec<&str> = structure.entries().iter().map(TreeEntry::path).collect();
        assert_eq!(paths, ["a", "a/b", "a/b/c.psd"]);

        for path in ["a", "a/b"] {
            assert_eq!(
                structure.entry(path).map(TreeEntry::kind),
                Some(EntryKind::Directory)
            );
        }
    }

    #[test]
    fn the_same_files_make_the_same_bytes_whatever_order_they_arrive_in() {
        let one = TreeStructure::build(
            None,
            [
                TreeEntry::file("b.psd", FileMode::Regular, key(1)),
                TreeEntry::file("a.psd", FileMode::Regular, key(2)),
            ],
        )
        .unwrap();
        let two = TreeStructure::build(
            None,
            [
                TreeEntry::file("a.psd", FileMode::Regular, key(2)),
                TreeEntry::file("b.psd", FileMode::Regular, key(1)),
            ],
        )
        .unwrap();

        assert_eq!(one, two);
        assert_eq!(one.hash(), two.hash());
        assert_eq!(one.encode(), two.encode());
    }

    #[test]
    fn an_empty_structure_is_a_structure() {
        let structure = TreeStructure::empty();

        assert!(structure.is_empty());
        assert_eq!(
            TreeStructure::decode(&structure.encode()).unwrap(),
            structure
        );
    }

    #[test]
    fn a_path_a_tree_may_not_name_is_refused() {
        for path in [
            "",
            ".",
            "..",
            "a/../b",
            "/a",
            ".rola/x",
            "a/.rola-tree",
            "x~theirs.psd",
        ] {
            assert!(
                TreeStructure::build(None, [TreeEntry::directory(path)]).is_err(),
                "{path}"
            );
        }

        // A rules file is a path like any other, at the tree root or under a directory.
        assert!(TreeStructure::build(None, [TreeEntry::directory(".rolaignore")]).is_ok());
        assert!(
            TreeStructure::build(
                None,
                [
                    TreeEntry::directory("a"),
                    TreeEntry::directory("a/.rolaignore")
                ]
            )
            .is_ok()
        );
    }

    #[test]
    fn a_path_named_twice_is_refused() {
        let error =
            TreeStructure::build(None, [TreeEntry::directory("a"), TreeEntry::directory("a")])
                .unwrap_err();

        assert_eq!(error, TreeError::Duplicate("a".to_owned()));
    }

    #[test]
    fn a_path_under_a_file_is_refused() {
        let error = TreeStructure::build(
            None,
            [
                TreeEntry::file("a", FileMode::Regular, key(1)),
                TreeEntry::directory("a/b"),
            ],
        )
        .unwrap_err();

        assert_eq!(error, TreeError::Conflict("a".to_owned()));
    }

    #[test]
    fn an_entry_that_is_not_what_its_kind_is_made_of_is_refused() {
        for entry in [
            TreeEntry::file("a", FileMode::Regular, key(1)).with_storage(None),
            TreeEntry::directory("a").with_target(Some("b".to_owned())),
            TreeEntry::symlink("a", ""),
        ] {
            assert!(validate_entry(&entry).is_err(), "{entry:?}");
        }
    }

    #[test]
    fn a_structure_whose_directory_is_not_named_is_not_read_back() {
        let bytes = raw(
            &[TreeEntry::file("a/b.psd", FileMode::Regular, key(1))],
            None,
        );

        assert_eq!(TreeStructure::decode(&bytes), Err(TreeError::Malformed));
    }

    #[test]
    fn a_structure_that_is_not_ordered_or_not_whole_is_not_read_back() {
        let bytes = raw(
            &[TreeEntry::directory("b"), TreeEntry::directory("a")],
            None,
        );
        assert_eq!(TreeStructure::decode(&bytes), Err(TreeError::Malformed));

        let mut truncated = raw(&[TreeEntry::directory("a")], None);
        truncated.pop();
        assert_eq!(TreeStructure::decode(&truncated), Err(TreeError::Malformed));

        let mut extra = raw(&[TreeEntry::directory("a")], None);
        extra.push(0);
        assert_eq!(TreeStructure::decode(&extra), Err(TreeError::Malformed));
    }

    #[test]
    fn a_magic_or_version_this_build_does_not_know_is_refused() {
        let mut bytes = TreeStructure::empty().encode();
        bytes[0] = b'X';
        assert_eq!(TreeStructure::decode(&bytes), Err(TreeError::Malformed));

        let mut bytes = TreeStructure::empty().encode();
        bytes[MAGIC.len()] = VERSION.wrapping_add(1);
        assert_eq!(TreeStructure::decode(&bytes), Err(TreeError::Malformed));
    }

    impl TreeEntry {
        /// The same entry with a different storage, for a test that wants one entry to be wrong.
        fn with_storage(mut self, storage: Option<Key>) -> Self {
            self.storage = storage;

            self
        }

        /// The same entry with a different target, for a test that wants one entry to be wrong.
        fn with_target(mut self, target: Option<String>) -> Self {
            self.target = target;

            self
        }
    }

    /// The keys a structure names are its description and what its files hold, each once.
    #[test]
    fn content_keys_names_the_description_and_the_files() {
        let description = key(9);
        let structure = TreeStructure::build(
            Some(description),
            [
                TreeEntry::file("a.psd", FileMode::Regular, key(1)),
                TreeEntry::file("b.psd", FileMode::Regular, key(2)),
                TreeEntry::directory("sub"),
            ],
        )
        .unwrap();

        let keys = structure.content_keys();

        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&description));
        assert!(keys.contains(&key(1)));
        assert!(keys.contains(&key(2)));
    }

    /// A subtree holds what a directory held, with the paths taken relative to it.
    #[test]
    fn a_subtree_is_what_a_directory_held() {
        let structure = TreeStructure::build(
            None,
            [
                TreeEntry::file("a.psd", FileMode::Regular, key(1)),
                TreeEntry::file("sub/b.psd", FileMode::Regular, key(2)),
            ],
        )
        .unwrap();

        let sub = structure.subtree("sub");

        assert!(sub.entry("a.psd").is_none());
        assert!(sub.entry("b.psd").is_some());
    }
}
