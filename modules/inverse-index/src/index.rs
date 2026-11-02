//! The inverse index itself: where it sits, how it is built, and how it answers.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use rorolala_storage::Blake3Hash;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vcs::{Hash, ROOT_VERSION, VCSIndex, VCSIndexObject, Variant, Version};

use crate::{Edge, InverseIndexError, InverseIndexReadingError, Snapshot, Table};

/// The directory, inside an index, the inverse index is kept in.
const INVERSE_DIR: &str = "inverse";

/// The file the inverse index is kept in.
const INVERSE_FILE: &str = "inverse_0.dat";

/// The reverse dependency index over one [`VCSIndex`]
///
/// It answers what a content-addressed index cannot — which objects depend on a key — from records
/// kept beside the objects, and falls back to reading the objects when the records do not describe
/// the index as it now stands. The two are the same answer; which one is read is this crate's
/// business, not the caller's.
#[lazyffi(export = RolaInverseIndex)]
#[derive(Clone)]
pub struct InverseIndex {
    /// The index whose objects the records are about.
    index: VCSIndex,
    /// The file the records are kept in.
    path: PathBuf,
}

impl InverseIndex {
    /// The inverse index of `index`, whether or not one has been built.
    #[must_use]
    pub fn at(index: VCSIndex) -> Self {
        let path = index.get_root().join(INVERSE_DIR).join(INVERSE_FILE);

        Self { index, path }
    }

    /// The inverse index of the Vault or Workspace `cwd` is inside, if it is inside one.
    #[must_use]
    pub fn locate(cwd: &Path) -> Option<Self> {
        VCSIndex::locate(cwd).map(Self::at)
    }

    /// The index whose objects these records are about.
    #[must_use]
    pub const fn index(&self) -> &VCSIndex {
        &self.index
    }

    /// The directory the index keeps its objects in.
    #[must_use]
    pub fn get_root(&self) -> &Path {
        self.index.get_root()
    }

    /// Brings the records up to date with the objects the index holds, reading only what is new.
    ///
    /// A rebuild is the one way the records are made. What it has to do follows what moved since it
    /// last ran: nothing, when the records already describe the index; the objects written since,
    /// which are read and added to the records it already holds; or every object, when the records
    /// cannot be added to — they are not there, do not read, are of a format this build does not
    /// know, or describe objects the index no longer holds. What it writes describes exactly the
    /// objects it read, so a reader that asked for it is answered from the answer, and a reader that
    /// arrives after another object is written sees records that no longer describe the index and
    /// falls back to the objects.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexError`] if the index cannot be read or the records cannot be written.
    pub async fn rebuild(&self) -> Result<InverseIndexReport, InverseIndexError> {
        let current = self.index.object_keys().await?;

        let Some(bytes) = self.read_file().await? else {
            return self.rebuild_full().await;
        };
        let Some(mut table) = Table::decode(&bytes).ok() else {
            return self.rebuild_full().await;
        };

        match mode_of(&table, &current) {
            RebuildMode::Fresh => Ok(report_fresh(&table, &bytes, current.len())),
            RebuildMode::Incremental => self.rebuild_incremental(&mut table, &current).await,
            RebuildMode::Full => self.rebuild_full().await,
        }
    }

    /// Builds the records afresh from every object the index holds, and writes them.
    ///
    /// This is what [`rebuild`](Self::rebuild) falls back to when the records cannot be added to,
    /// and what makes an index's records from nothing. It costs a read per object, so what a caller
    /// that just wants the records brought up to date should ask for is [`rebuild`](Self::rebuild).
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexError`] if the index cannot be read or the records cannot be written.
    pub async fn rebuild_full(&self) -> Result<InverseIndexReport, InverseIndexError> {
        let objects = self.index.read_objects().await?;
        let (table, counted) = records(&objects);
        let encoded = table.encode();
        let report = report_written(
            RebuildMode::Full,
            &table,
            &encoded,
            counted.objects,
            counted,
        );

        self.write(&encoded).await?;

        Ok(report)
    }

    /// Adds the objects written since the records were made, and writes the records again.
    async fn rebuild_incremental(
        &self,
        table: &mut Table,
        current: &[Hash],
    ) -> Result<InverseIndexReport, InverseIndexError> {
        let added = added_keys(table.covered(), current);
        let mut counted = Counting {
            objects: current.len(),
            ..Counting::default()
        };

        for key in &added {
            match self.index.read(*key).await? {
                VCSIndexObject::Variant(variant) => {
                    add_variant_edges(table, *key, &variant);
                    counted.variants = counted.variants.saturating_add(1);
                }
                VCSIndexObject::Version(version) => {
                    add_version_edges(table, *key, &version);
                    counted.versions = counted.versions.saturating_add(1);
                    counted.roots = counted.roots.saturating_add(usize::from(version.is_root()));
                }
                VCSIndexObject::Creator(_) => {
                    counted.creators = counted.creators.saturating_add(1);
                }
                VCSIndexObject::Message(_) => {
                    counted.messages = counted.messages.saturating_add(1);
                }
            }
        }

        counted.numbered = counted.roots.saturating_add(count_numbered(table));
        table.set_covered(current.to_vec());

        let encoded = table.encode();
        let report = report_written(
            RebuildMode::Incremental,
            table,
            &encoded,
            counted.objects,
            counted,
        );

        self.write(&encoded).await?;

        Ok(report)
    }

    /// The records, when there are some that describe the index as it now stands.
    ///
    /// A file that is not there, does not read, or was written before an object was added or taken
    /// away is no answer at all: what it describes is not what the index holds, so a caller asks the
    /// objects instead. That is what makes the records a cache rather than a second truth.
    async fn load(&self) -> Result<Option<Table>, InverseIndexError> {
        let Some(bytes) = self.read_file().await? else {
            return Ok(None);
        };
        let Ok(table) = Table::decode(&bytes) else {
            return Ok(None);
        };

        let current = self.index.object_keys().await?;

        Ok(table.is_fresh(&current).then_some(table))
    }

    /// The bytes the records are kept in, when the file is there.
    async fn read_file(&self) -> Result<Option<Vec<u8>>, InverseIndexError> {
        match tokio::fs::read(&self.path).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Writes `bytes` whole, laid down beside their place and moved into it.
    async fn write(&self, bytes: &[u8]) -> Result<(), InverseIndexError> {
        let parent = self.path.parent().ok_or(InverseIndexError::Malformed)?;
        tokio::fs::create_dir_all(parent).await?;

        let temporary = self
            .path
            .with_file_name(format!("{INVERSE_FILE}.{}.tmp", std::process::id()));
        tokio::fs::write(&temporary, bytes).await?;
        tokio::fs::rename(&temporary, &self.path).await?;

        Ok(())
    }

    /// The records read the other way, when they describe the index as it now stands.
    ///
    /// A snapshot is one inversion of the records: every variant by its key with the parts it names,
    /// every version by its key with the variant it fixes and its number, and the questions the
    /// graph asks of them — which versions fix a variant, which variants are based on a version,
    /// which merge one in, which were made of one content. It is what a reader that walks asks for
    /// instead of one record at a time.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexError`] if the index cannot be read, or if what is stored under a key
    /// cannot be read as the records this build wrote.
    pub async fn snapshot(&self) -> Result<Option<Snapshot>, InverseIndexError> {
        let Some(table) = self.load().await? else {
            return Ok(None);
        };

        Snapshot::from_table(&table).map(Some)
    }

    /// The records a reading of the objects gives, without keeping them.
    ///
    /// This is what the records would say if they were built now, at the price of reading every
    /// object: the answer for a run that may not write the records, such as a listing, when they do
    /// not describe the index. It is the same answer [`snapshot`](Self::snapshot) gives once a
    /// rebuild has made the records.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexError`] if the index cannot be read.
    pub async fn scan(&self) -> Result<Snapshot, InverseIndexError> {
        let objects = self.index.read_objects().await?;
        let (table, _) = records(&objects);

        Snapshot::from_table(&table)
    }

    /// The variants made from the content stored under `key`.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexReadingError`] if the index cannot be read.
    pub async fn store_dependents(&self, key: Hash) -> Result<Vec<Hash>, InverseIndexReadingError> {
        if let Some(table) = self.load().await.map_err(reading)? {
            return Ok(table.dependents(key, Edge::Storage));
        }

        direct::store_dependents(&self.index, key).await
    }

    /// The versions that fix the variant named `key`.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexReadingError`] if the index cannot be read.
    pub async fn variant_dependents(
        &self,
        key: Hash,
    ) -> Result<Vec<Hash>, InverseIndexReadingError> {
        if let Some(table) = self.load().await.map_err(reading)? {
            return Ok(table.dependents(key, Edge::Variant));
        }

        direct::variant_dependents(&self.index, key).await
    }

    /// The variants based on the version named `key`.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexReadingError`] if the index cannot be read.
    pub async fn version_dependents(
        &self,
        key: Hash,
    ) -> Result<Vec<Hash>, InverseIndexReadingError> {
        if let Some(table) = self.load().await.map_err(reading)? {
            return Ok(table.dependents(key, Edge::Base));
        }

        direct::version_dependents(&self.index, key).await
    }

    /// The versions based on the version named `key`
    ///
    /// Each is the version fixed by a variant that is based on `key`; a variant with no version yet
    /// contributes nothing, since a version is the thing that is numbered.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexReadingError`] if the index cannot be read.
    pub async fn child_versions(&self, key: Hash) -> Result<Vec<Hash>, InverseIndexReadingError> {
        if let Some(table) = self.load().await.map_err(reading)? {
            let mut children = Vec::new();
            for variant in table.dependents(key, Edge::Base) {
                children.extend(table.dependents(variant, Edge::Variant));
            }
            children.sort_unstable();
            children.dedup();

            return Ok(children);
        }

        direct::child_versions(&self.index, key).await
    }

    /// The variants made by the creator named `key`.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexReadingError`] if the index cannot be read.
    pub async fn creator_dependents(
        &self,
        key: Hash,
    ) -> Result<Vec<Hash>, InverseIndexReadingError> {
        if let Some(table) = self.load().await.map_err(reading)? {
            return Ok(table.dependents(key, Edge::Creator));
        }

        direct::creator_dependents(&self.index, key).await
    }

    /// The variants that say what the message named `key` says.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexReadingError`] if the index cannot be read.
    pub async fn message_dependents(
        &self,
        key: Hash,
    ) -> Result<Vec<Hash>, InverseIndexReadingError> {
        if let Some(table) = self.load().await.map_err(reading)? {
            return Ok(table.dependents(key, Edge::Message));
        }

        direct::message_dependents(&self.index, key).await
    }

    /// The variants that merge in the variant named `key`.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexReadingError`] if the index cannot be read.
    pub async fn join_dependents(&self, key: Hash) -> Result<Vec<Hash>, InverseIndexReadingError> {
        if let Some(table) = self.load().await.map_err(reading)? {
            return Ok(table.dependents(key, Edge::Join));
        }

        direct::join_dependents(&self.index, key).await
    }

    /// The number of the version named `key`
    ///
    /// The records answer with the number they hold; when they hold none — because the version was
    /// not there when they were built, or its chain does not reach the root — the chain is walked
    /// from the objects, which is what tells a hole from a version that is simply not one.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexReadingError::NotFound`] if nothing is stored under `key` or `key` is
    /// not a version, and otherwise what reading the index failed with.
    pub async fn version_num(&self, key: Hash) -> Result<u64, InverseIndexReadingError> {
        if let Some(table) = self.load().await.map_err(reading)?
            && let Some(number) = table.number(key)
        {
            return Ok(number);
        }

        direct::version_num(&self.index, key).await
    }
}

/// What a rebuild found, and what it wrote.
#[lazyffi(export = RolaInverseIndexReport)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InverseIndexReport {
    /// What the rebuild had to do: nothing, add what was new, or read every object.
    pub mode: RebuildMode,
    /// How many objects the index holds, as the build left it.
    pub objects: u64,
    /// How many objects this rebuild read.
    pub objects_read: u64,
    /// How many of the objects this rebuild read are variants.
    pub variants: u64,
    /// How many of them are versions.
    pub versions: u64,
    /// How many are creators.
    pub creators: u64,
    /// How many are messages.
    pub messages: u64,
    /// How many versions this rebuild numbered itself, the root's own number included.
    pub versions_numbered: u64,
    /// How many records the inverse index has.
    pub entries: u64,
    /// How many dependents are noted across the records.
    pub dependents: u64,
    /// The digest of what was written, as hex.
    pub fingerprint: String,
}

/// What a rebuild has to do, and so what it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RebuildMode {
    /// Nothing: the records describe the index.
    Fresh,
    /// The index holds objects the records do not: they were read and added.
    Incremental,
    /// The records do not describe the index at all: every object was read.
    Full,
}

impl RebuildMode {
    /// The mode, as the report and `--json` spell it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Incremental => "incremental",
            Self::Full => "full",
        }
    }
}

/// What reading the objects found, for the report a rebuild answers with.
#[derive(Debug, Default, Clone, Copy)]
struct Counting {
    /// How many objects the index holds.
    objects: usize,
    /// How many variants were read.
    variants: usize,
    /// How many versions were read.
    versions: usize,
    /// How many creators were read.
    creators: usize,
    /// How many messages were read.
    messages: usize,
    /// How many of the versions read are the root.
    roots: usize,
    /// How many versions were numbered by this rebuild.
    numbered: usize,
}

/// What a rebuild has to do for the records to describe `current`.
fn mode_of(table: &Table, current: &[Hash]) -> RebuildMode {
    if table.is_fresh(current) {
        return RebuildMode::Fresh;
    }

    if is_appendable(table.covered(), current) {
        RebuildMode::Incremental
    } else {
        RebuildMode::Full
    }
}

/// Whether `covered` is `current` with objects added: every key the records hold is still there.
///
/// Both are in key order, which is what makes this one pass. A key the records hold that the index
/// no longer does is not something they can be added to — an object once written is not taken away
/// by a rebuild that only adds — so it is the whole table that is made again.
fn is_appendable(covered: &[Hash], current: &[Hash]) -> bool {
    let mut held = covered.iter();
    let mut next = held.next();

    for key in current {
        match next {
            Some(held_key) if held_key == key => next = held.next(),
            Some(held_key) if held_key < key => return false,
            _ => {}
        }
    }

    next.is_none()
}

/// The keys of `current` the records do not cover, in key order.
fn added_keys(covered: &[Hash], current: &[Hash]) -> Vec<Hash> {
    let mut held = covered.iter();
    let mut next = held.next();
    let mut added = Vec::new();

    for key in current {
        if next.is_some_and(|held_key| held_key == key) {
            next = held.next();
        } else {
            added.push(*key);
        }
    }

    added
}

/// The records a reading of `objects` gives, and what reading them found.
fn records(objects: &[(Hash, VCSIndexObject)]) -> (Table, Counting) {
    let mut covered: Vec<Hash> = objects.iter().map(|(key, _)| *key).collect();
    covered.sort_unstable();
    covered.dedup();

    let mut table = Table::new(covered);
    let mut counted = Counting {
        objects: objects.len(),
        ..Counting::default()
    };

    for (key, object) in objects {
        match object {
            VCSIndexObject::Variant(variant) => {
                add_variant_edges(&mut table, *key, variant);
                counted.variants = counted.variants.saturating_add(1);
            }
            VCSIndexObject::Version(version) => {
                add_version_edges(&mut table, *key, version);
                counted.versions = counted.versions.saturating_add(1);
                counted.roots = counted.roots.saturating_add(usize::from(version.is_root()));
            }
            VCSIndexObject::Creator(_) => {
                counted.creators = counted.creators.saturating_add(1);
            }
            VCSIndexObject::Message(_) => {
                counted.messages = counted.messages.saturating_add(1);
            }
        }
    }

    counted.numbered = counted.roots.saturating_add(count_numbered(&mut table));

    (table, counted)
}

/// The report of a rebuild that wrote `encoded`.
fn report_written(
    mode: RebuildMode,
    table: &Table,
    encoded: &[u8],
    objects: usize,
    counted: Counting,
) -> InverseIndexReport {
    InverseIndexReport {
        mode,
        objects: width(objects),
        objects_read: width(counted.read()),
        variants: width(counted.variants),
        versions: width(counted.versions),
        creators: width(counted.creators),
        messages: width(counted.messages),
        versions_numbered: width(counted.numbered),
        entries: width(table.entry_count()),
        dependents: width(table.edge_count()),
        fingerprint: Hash::new(*blake3::hash(encoded).as_bytes()).hex(),
    }
}

/// The report of a rebuild that found the records already describing the index.
fn report_fresh(table: &Table, bytes: &[u8], objects: usize) -> InverseIndexReport {
    InverseIndexReport {
        mode: RebuildMode::Fresh,
        objects: width(objects),
        objects_read: 0,
        variants: 0,
        versions: 0,
        creators: 0,
        messages: 0,
        versions_numbered: 0,
        entries: width(table.entry_count()),
        dependents: width(table.edge_count()),
        fingerprint: Hash::new(*blake3::hash(bytes).as_bytes()).hex(),
    }
}

impl Counting {
    /// How many objects were read.
    const fn read(&self) -> usize {
        self.variants
            .saturating_add(self.versions)
            .saturating_add(self.creators)
            .saturating_add(self.messages)
    }
}

/// Numbers every version whose base version has a number, keeping the ones already numbered.
///
/// A version's number is its base version's, one past, and the root is numbered before the first; so
/// the numbers fill from whatever is numbered outwards. A version cut off from every numbered one by
/// a hole — a base version that was never written — is left without a number rather than given one
/// that is not the chain's, and a later rebuild fills it when the link arrives. A number, once
/// filled, is never changed: a version's base is fixed, so the answer is fixed with it.
///
/// Returns how many versions were numbered.
fn count_numbered(table: &mut Table) -> usize {
    let mut queue: VecDeque<(Hash, u64)> = table.numbered().collect();
    let mut numbered = 0_usize;

    while let Some((version, number)) = queue.pop_front() {
        let next = number.wrapping_add(1);

        for variant in table.dependents(version, Edge::Base) {
            for child in table.dependents(variant, Edge::Variant) {
                if table.number(child).is_some() {
                    continue;
                }

                table.set_number(child, next);
                queue.push_back((child, next));
                numbered = numbered.saturating_add(1);
            }
        }
    }

    numbered
}

/// Notes the edges a Variant has, under the key it is held by.
fn add_variant_edges(table: &mut Table, key: Hash, variant: &Variant) {
    for (edge, target) in variant_edges(variant) {
        table.add(target, key, edge);
    }
}

/// Notes the edge a Version has, under the key it is held by, and its number when it is the root.
fn add_version_edges(table: &mut Table, key: Hash, version: &Version) {
    let (edge, target) = version_edge(version);
    table.add(target, key, edge);

    if version.is_root() {
        table.set_number(key, ROOT_VERSION);
    }
}

/// The key a digest names.
const fn key_of(digest: &Blake3Hash) -> Hash {
    Hash::new(*digest)
}

/// How a variant points at its parts, in one place.
fn variant_edges(variant: &Variant) -> Vec<(Edge, Hash)> {
    let mut edges = vec![
        (Edge::Storage, key_of(variant.storage_hash())),
        (Edge::Base, key_of(variant.base_version())),
        (Edge::Creator, key_of(variant.creator())),
        (Edge::Message, key_of(variant.message())),
    ];
    if let Some(join) = variant.join() {
        edges.push((Edge::Join, key_of(join)));
    }

    edges
}

/// The key the Version `version` fixes.
const fn version_edge(version: &Version) -> (Edge, Hash) {
    (Edge::Variant, key_of(version.variant()))
}

/// `len` as the width a report counts in.
fn width(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

/// What reading through the records failed with, told the way a reading error is.
fn reading(error: InverseIndexError) -> InverseIndexReadingError {
    match error {
        InverseIndexError::Read(source) => InverseIndexReadingError::from(source),
        InverseIndexError::Malformed => InverseIndexReadingError::Malformed,
        InverseIndexError::Io(source) => InverseIndexReadingError::Io(source),
    }
}

/// The answers read from the objects, when the records do not describe the index.
///
/// These are the same questions the records answer, put to the objects directly: every object the
/// index holds is read and the ones that point at the key are kept. It costs a read per object,
/// which is what the records are for — this is the answer that is always right, to fall back to.
mod direct {
    use std::collections::BTreeMap;

    use rorolala_vcs::{Hash, VCSIndex, VCSIndexObject};

    use super::key_of;
    use crate::InverseIndexReadingError;

    /// The variants made from the content stored under `key`.
    pub(super) async fn store_dependents(
        index: &VCSIndex,
        key: Hash,
    ) -> Result<Vec<Hash>, InverseIndexReadingError> {
        let mut found = Vec::new();
        for (hash, object) in index.read_objects().await? {
            if let VCSIndexObject::Variant(variant) = object
                && key_of(variant.storage_hash()) == key
            {
                found.push(hash);
            }
        }

        Ok(found)
    }

    /// The versions that fix the variant named `key`.
    pub(super) async fn variant_dependents(
        index: &VCSIndex,
        key: Hash,
    ) -> Result<Vec<Hash>, InverseIndexReadingError> {
        let mut found = Vec::new();
        for (hash, object) in index.read_objects().await? {
            if let VCSIndexObject::Version(version) = object
                && key_of(version.variant()) == key
            {
                found.push(hash);
            }
        }

        Ok(found)
    }

    /// The variants based on the version named `key`.
    pub(super) async fn version_dependents(
        index: &VCSIndex,
        key: Hash,
    ) -> Result<Vec<Hash>, InverseIndexReadingError> {
        let mut found = Vec::new();
        for (hash, object) in index.read_objects().await? {
            if let VCSIndexObject::Variant(variant) = object
                && key_of(variant.base_version()) == key
            {
                found.push(hash);
            }
        }

        Ok(found)
    }

    /// The versions based on the version named `key`.
    pub(super) async fn child_versions(
        index: &VCSIndex,
        key: Hash,
    ) -> Result<Vec<Hash>, InverseIndexReadingError> {
        let objects = index.read_objects().await?;

        // Which version fixes a variant: a version is numbered by the variant it points at, so the
        // two are put together before the variants based on `key` are asked for their version.
        let mut pinned: BTreeMap<Hash, Hash> = BTreeMap::new();
        for (hash, object) in &objects {
            if let VCSIndexObject::Version(version) = object {
                pinned.insert(key_of(version.variant()), *hash);
            }
        }

        let mut children = Vec::new();
        for (hash, object) in &objects {
            if let VCSIndexObject::Variant(variant) = object
                && key_of(variant.base_version()) == key
                && let Some(version) = pinned.get(hash)
            {
                children.push(*version);
            }
        }

        children.sort_unstable();
        children.dedup();

        Ok(children)
    }

    /// The variants made by the creator named `key`.
    pub(super) async fn creator_dependents(
        index: &VCSIndex,
        key: Hash,
    ) -> Result<Vec<Hash>, InverseIndexReadingError> {
        let mut found = Vec::new();
        for (hash, object) in index.read_objects().await? {
            if let VCSIndexObject::Variant(variant) = object
                && key_of(variant.creator()) == key
            {
                found.push(hash);
            }
        }

        Ok(found)
    }

    /// The variants that say what the message named `key` says.
    pub(super) async fn message_dependents(
        index: &VCSIndex,
        key: Hash,
    ) -> Result<Vec<Hash>, InverseIndexReadingError> {
        let mut found = Vec::new();
        for (hash, object) in index.read_objects().await? {
            if let VCSIndexObject::Variant(variant) = object
                && key_of(variant.message()) == key
            {
                found.push(hash);
            }
        }

        Ok(found)
    }

    /// The variants that merge in the variant named `key`.
    pub(super) async fn join_dependents(
        index: &VCSIndex,
        key: Hash,
    ) -> Result<Vec<Hash>, InverseIndexReadingError> {
        let mut found = Vec::new();
        for (hash, object) in index.read_objects().await? {
            if let VCSIndexObject::Variant(variant) = object
                && variant.join().is_some_and(|join| key_of(join) == key)
            {
                found.push(hash);
            }
        }

        Ok(found)
    }

    /// The number of the version named `key`, walked from the objects.
    pub(super) async fn version_num(
        index: &VCSIndex,
        key: Hash,
    ) -> Result<u64, InverseIndexReadingError> {
        let object = index.read(key).await?;
        let version = object
            .expect_version()
            .map_err(|_| InverseIndexReadingError::Malformed)?;

        Ok(index.version_num(&version).await?)
    }
}

#[cfg(test)]
mod tests {
    use rorolala_vcs::{Hash, ROOT_VERSION};

    use super::{RebuildMode, count_numbered, is_appendable, mode_of};
    use crate::{Edge, Table};

    #[test]
    fn an_edge_round_trips_through_its_byte() {
        for edge in [
            Edge::Storage,
            Edge::Base,
            Edge::Join,
            Edge::Creator,
            Edge::Message,
            Edge::Variant,
        ] {
            assert_eq!(Edge::from_id(edge.id()), Some(edge));
        }

        assert_eq!(Edge::from_id(6), None);
    }

    #[test]
    fn a_table_round_trips_through_its_bytes() {
        let covered = vec![Hash::new([1_u8; 32]), Hash::new([2_u8; 32])];
        let mut table = Table::new(covered.clone());
        table.add(Hash::new([1_u8; 32]), Hash::new([3_u8; 32]), Edge::Storage);
        table.set_number(Hash::new([2_u8; 32]), 7);

        let back = Table::decode(&table.encode()).unwrap();

        assert_eq!(back.covered(), covered.as_slice());
        assert_eq!(
            back.dependents(Hash::new([1_u8; 32]), Edge::Storage),
            vec![Hash::new([3_u8; 32])]
        );
        assert_eq!(back.number(Hash::new([2_u8; 32])), Some(7));
        assert!(back.is_fresh(&covered));
        assert_eq!(back.targets().count(), 2);
        assert_eq!(back.dependents_all(Hash::new([1_u8; 32])).count(), 1);
        assert_eq!(back.numbered().count(), 1);
        assert_eq!(back.numbered().next(), Some((Hash::new([2_u8; 32]), 7)));

        // One object fewer than it describes is one object moved on: the records are no answer.
        assert!(!back.is_fresh(&covered[..1]));
    }

    #[test]
    fn what_a_rebuild_has_to_do_follows_what_moved() {
        let one = Hash::new([0x40; 32]);
        let two = Hash::new([0x41; 32]);
        let three = Hash::new([0x42; 32]);
        let table = Table::new(vec![one, two]);

        assert_eq!(mode_of(&table, &[one, two]), RebuildMode::Fresh);
        assert_eq!(
            mode_of(&table, &[one, two, three]),
            RebuildMode::Incremental
        );
        assert_eq!(mode_of(&table, &[one]), RebuildMode::Full);
        assert_eq!(mode_of(&table, &[one, three]), RebuildMode::Full);
        assert_eq!(mode_of(&table, &[two, three]), RebuildMode::Full);
        assert_eq!(
            mode_of(&table, &[one, two, three]),
            RebuildMode::Incremental
        );
        assert_eq!(
            mode_of(&table, &[one, two, three]).name(),
            "incremental",
            "a mode says what it is called",
        );
    }

    #[test]
    fn an_append_needs_every_key_the_records_hold() {
        let one = Hash::new([0x40; 32]);
        let two = Hash::new([0x41; 32]);

        assert!(is_appendable(&[], &[one, two]));
        assert!(is_appendable(&[one], &[one, two]));
        assert!(is_appendable(&[one, two], &[one, two]));
        assert!(!is_appendable(&[one], &[two]));
        assert!(!is_appendable(&[two], &[one]));
        assert!(!is_appendable(&[one, two], &[one]));
    }

    #[test]
    fn a_number_fills_from_the_root_outwards() {
        let root = Hash::new([0x10; 32]);
        let root_variant = Hash::new([0x11; 32]);
        let variant = Hash::new([0x12; 32]);
        let version = Hash::new([0x13; 32]);

        // The root fixes a variant of its own; the first variant is based on the root, and the
        // first version fixes that variant.
        let mut table = Table::new(vec![]);
        table.add(root_variant, root, Edge::Variant);
        table.add(root, variant, Edge::Base);
        table.add(variant, version, Edge::Variant);
        table.set_number(root, ROOT_VERSION);

        assert_eq!(count_numbered(&mut table), 1);
        assert_eq!(table.number(root), Some(ROOT_VERSION));
        assert_eq!(table.number(version), Some(0));
    }

    #[test]
    fn a_chain_cut_off_from_the_root_is_left_unnumbered() {
        // A version based on a version that was never written: the chain does not reach the root,
        // so no number is filled rather than one that is not the chain's.
        let missing = Hash::new([0xab; 32]);
        let variant = Hash::new([0x20; 32]);
        let version = Hash::new([0x21; 32]);

        let mut table = Table::new(vec![]);
        table.add(missing, variant, Edge::Base);
        table.add(variant, version, Edge::Variant);

        assert_eq!(count_numbered(&mut table), 0);
        assert_eq!(table.number(version), None);
    }

    #[test]
    fn a_number_fills_once_the_link_it_waited_for_arrives() {
        let root = Hash::new([0x30; 32]);
        let first = Hash::new([0x31; 32]);
        let first_variant = Hash::new([0x32; 32]);
        let second = Hash::new([0x33; 32]);
        let second_variant = Hash::new([0x34; 32]);

        // The second version is in the records, based on the first — which is not there yet.
        let mut table = Table::new(vec![]);
        table.add(first, second_variant, Edge::Base);
        table.add(second_variant, second, Edge::Variant);
        assert_eq!(count_numbered(&mut table), 0);

        // The first version and the root arrive: this rebuild numbers both from the root outwards.
        table.add(root, first_variant, Edge::Base);
        table.add(first_variant, first, Edge::Variant);
        table.set_number(root, ROOT_VERSION);

        assert_eq!(count_numbered(&mut table), 2);
        assert_eq!(table.number(first), Some(0));
        assert_eq!(table.number(second), Some(1));

        // A number once written is not written again.
        assert_eq!(count_numbered(&mut table), 0);
    }
}
