//! The inverse index itself: where it sits, how it is built, and how it answers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rorolala_storage::Blake3Hash;
use rorolala_utils_lazyffi::lazyffi;
use rorolala_utils_location::Locate as _;
use rorolala_vcs::{Hash, ROOT_VERSION, VCSIndex, VCSIndexObject, Variant, Version};

use crate::{Edge, InverseIndexError, InverseIndexReadingError, Table};

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

    /// Builds the records afresh from every object the index holds, and writes them.
    ///
    /// A rebuild is the one way the records are made: reading every object once, noting what each
    /// points at, and working out each version's number from the chain. What it writes describes
    /// exactly the objects it read, so a reader that asked for it is answered from it, and a reader
    /// that arrives after another object is written sees records that no longer describe the index
    /// and falls back to the objects.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexError`] if the index cannot be read or the records cannot be written.
    pub async fn rebuild(&self) -> Result<InverseIndexReport, InverseIndexError> {
        let objects = self.index.read_objects().await?;

        let mut covered: Vec<Hash> = objects.iter().map(|(key, _)| *key).collect();
        covered.sort_unstable();
        covered.dedup();

        let mut table = Table::new(covered);
        let mut variants: BTreeMap<Hash, Variant> = BTreeMap::new();
        let mut versions: BTreeMap<Hash, Version> = BTreeMap::new();
        let mut report = InverseIndexReport::default();

        for (key, object) in &objects {
            match object {
                VCSIndexObject::Variant(variant) => {
                    for (edge, target) in variant_edges(variant) {
                        table.add(target, *key, edge);
                    }
                    variants.insert(*key, variant.clone());
                    report.variants = report.variants.saturating_add(1);
                }
                VCSIndexObject::Version(version) => {
                    let (edge, target) = version_edge(version);
                    table.add(target, *key, edge);
                    versions.insert(*key, version.clone());
                    report.versions = report.versions.saturating_add(1);
                }
                VCSIndexObject::Creator(_) => report.creators = report.creators.saturating_add(1),
                VCSIndexObject::Message(_) => report.messages = report.messages.saturating_add(1),
            }
        }

        number_versions(&mut table, &variants, &versions);

        report.objects = width(objects.len());
        report.entries = width(table.entry_count());
        report.dependents = width(table.edge_count());
        report.fingerprint = Hash::new(*blake3::hash(&table.encode()).as_bytes()).hex();

        self.write(&table).await?;

        Ok(report)
    }

    /// The records, when there are some that describe the index as it now stands.
    ///
    /// A file that is not there, does not read, or was written before an object was added or taken
    /// away is no answer at all: what it describes is not what the index holds, so a caller asks the
    /// objects instead. That is what makes the records a cache rather than a second truth.
    async fn load(&self) -> Result<Option<Table>, InverseIndexError> {
        let bytes = match tokio::fs::read(&self.path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };

        let Ok(table) = Table::decode(&bytes) else {
            return Ok(None);
        };

        let current = self.index.object_keys().await?;
        if !table.is_fresh(&current) {
            return Ok(None);
        }

        Ok(Some(table))
    }

    /// Writes `table` whole, laid down beside its place and moved into it.
    async fn write(&self, table: &Table) -> Result<(), InverseIndexError> {
        let parent = self.path.parent().ok_or(InverseIndexError::Malformed)?;
        tokio::fs::create_dir_all(parent).await?;

        let temporary = self
            .path
            .with_file_name(format!("{INVERSE_FILE}.{}.tmp", std::process::id()));
        tokio::fs::write(&temporary, table.encode()).await?;
        tokio::fs::rename(&temporary, &self.path).await?;

        Ok(())
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
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InverseIndexReport {
    /// How many objects the index holds.
    pub objects: u64,
    /// How many of them are variants.
    pub variants: u64,
    /// How many are versions.
    pub versions: u64,
    /// How many are creators.
    pub creators: u64,
    /// How many are messages.
    pub messages: u64,
    /// How many records the inverse index has.
    pub entries: u64,
    /// How many dependents are noted across the records.
    pub dependents: u64,
    /// The digest of what was written, as hex.
    pub fingerprint: String,
}

/// Fills in the number of every version whose chain to the root is whole.
///
/// A version's number is its base version's, one past, and the root is numbered from the start; so
/// the chain fills from the root outwards, over and over until nothing more can be filled. A
/// version cut off from the root by a hole — a base version that was never written — is left
/// without a number rather than given one that is not the chain's. A number, once filled, is never
/// changed: a version's base is fixed, so the answer is fixed with it.
fn number_versions(
    table: &mut Table,
    variants: &BTreeMap<Hash, Variant>,
    versions: &BTreeMap<Hash, Version>,
) {
    for (key, version) in versions {
        if version.is_root() {
            table.set_number(*key, ROOT_VERSION);
        }
    }

    loop {
        let mut progressed = false;

        for (key, version) in versions {
            if table.number(*key).is_some() {
                continue;
            }

            let Some(variant) = variants.get(&key_of(version.variant())) else {
                continue;
            };
            let Some(base) = table.number(key_of(variant.base_version())) else {
                continue;
            };

            table.set_number(*key, base.wrapping_add(1));
            progressed = true;
        }

        if !progressed {
            return;
        }
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
    use std::collections::BTreeMap;

    use rorolala_vcs::{Hash, ROOT_VERSION, VCSWrite as _, Version};

    use super::{key_of, number_versions};
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

        // One object fewer than it describes is one object moved on: the records are no answer.
        assert!(!back.is_fresh(&covered[..1]));
    }

    #[test]
    fn a_number_fills_from_the_root_outwards() {
        let root = Version::root();
        let variant = root.new_variant([1_u8; 32], [2_u8; 32], [3_u8; 32]);
        let first = variant.new_version();

        let mut versions = BTreeMap::new();
        versions.insert(root.hash(), root.clone());
        versions.insert(first.hash(), first.clone());
        let mut variants = BTreeMap::new();
        variants.insert(variant.hash(), variant.clone());

        let mut table = Table::new(vec![]);
        number_versions(&mut table, &variants, &versions);

        assert_eq!(table.number(root.hash()), Some(ROOT_VERSION));
        assert_eq!(table.number(first.hash()), Some(0));
        assert_eq!(key_of(variant.base_version()), root.hash());
    }

    #[test]
    fn a_chain_cut_off_from_the_root_is_left_unnumbered() {
        // A version based on a version that was never written: the chain does not reach the root,
        // so no number is filled rather than one that is not the chain's.
        let missing = Version::new_bare_version([0xab; 32], 0);
        let variant = missing.new_variant([1_u8; 32], [2_u8; 32], [3_u8; 32]);
        let version = variant.new_version();

        let mut versions = BTreeMap::new();
        versions.insert(version.hash(), version.clone());
        let mut variants = BTreeMap::new();
        variants.insert(variant.hash(), variant);

        let mut table = Table::new(vec![]);
        number_versions(&mut table, &variants, &versions);

        assert_eq!(table.number(version.hash()), None);
    }
}
