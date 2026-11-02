//! The records read the other way: what each object is, and the questions a walk asks of them.
//!
//! The records are kept as what points at what, because that is what a lookup asks. A reader that
//! walks — which versions fix a variant, which variants are based on a version, which merge one in,
//! which were made of one content — asks the other way, so a snapshot inverts every record once and
//! keeps the result: each variant by its key with the parts it names, each version by its key with
//! the variant it fixes and its number, and the reverse questions those make answerable without
//! reading the records again per step.

use std::collections::BTreeMap;

use rorolala_vcs::{Hash, ROOT_VERSION};

use crate::{Edge, InverseIndexError, Table};

/// What a variant is made of, as the records state it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VariantRecord {
    /// The version it is based on.
    pub base_version: Hash,
    /// The variant it merges in, when it is a merge.
    pub join: Option<Hash>,
    /// The content it was made of.
    pub storage: Hash,
    /// Who made it.
    pub creator: Hash,
    /// What it says was done.
    pub message: Hash,
}

/// What a version fixes, and its number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionRecord {
    /// The variant it fixes.
    ///
    /// The root points at no variant, so what is named here for it is the zero hash: `is_root` is
    /// what tells the two apart, and a reader drawing a chain starts at the root's own tab.
    pub variant: Hash,
    /// The number of the version in its chain, when the chain reaches a numbered one.
    ///
    /// `None` for the root, which is numbered before the first rather than in the chain, and for a
    /// version whose chain is cut off from the root.
    pub number: Option<u64>,
    /// Whether it is the root: the version every chain starts from.
    pub is_root: bool,
}

/// The records read the other way, ready to be walked.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// The variants the records name, by their key.
    variants: BTreeMap<Hash, VariantRecord>,
    /// The versions the records name, by their key.
    versions: BTreeMap<Hash, VersionRecord>,
    /// The versions fixing one variant, by the variant.
    versions_by_variant: BTreeMap<Hash, Vec<Hash>>,
    /// The variants based on one version, by the version.
    variants_by_base: BTreeMap<Hash, Vec<Hash>>,
    /// The variants merging one variant in, by the one merged in.
    variants_by_join: BTreeMap<Hash, Vec<Hash>>,
    /// The variants made of one content, by the storage address.
    variants_by_storage: BTreeMap<Hash, Vec<Hash>>,
}

/// A variant's parts while the records are being inverted.
#[derive(Debug, Default, Clone, Copy)]
struct VariantDraft {
    /// The version it is based on.
    base_version: Option<Hash>,
    /// The variant it merges in.
    join: Option<Hash>,
    /// The content it was made of.
    storage: Option<Hash>,
    /// Who made it.
    creator: Option<Hash>,
    /// What it says was done.
    message: Option<Hash>,
}

impl Snapshot {
    /// The snapshot of the records in `table`.
    ///
    /// # Errors
    ///
    /// Returns [`InverseIndexError::Malformed`] if a variant the records name does not have every
    /// part a variant is made of — which is not something this build writes, since it reads the
    /// parts off the object itself.
    pub(crate) fn from_table(table: &Table) -> Result<Self, InverseIndexError> {
        let mut drafts: BTreeMap<Hash, VariantDraft> = BTreeMap::new();
        let mut versions = BTreeMap::new();

        for target in table.targets() {
            for (source, edge) in table.dependents_all(target) {
                match edge {
                    Edge::Variant => {
                        let number = table.number(source);
                        let is_root = number == Some(ROOT_VERSION);
                        versions.insert(
                            source,
                            VersionRecord {
                                variant: target,
                                number: if is_root { None } else { number },
                                is_root,
                            },
                        );
                    }
                    Edge::Base => drafts.entry(source).or_default().base_version = Some(target),
                    Edge::Join => drafts.entry(source).or_default().join = Some(target),
                    Edge::Storage => drafts.entry(source).or_default().storage = Some(target),
                    Edge::Creator => drafts.entry(source).or_default().creator = Some(target),
                    Edge::Message => drafts.entry(source).or_default().message = Some(target),
                }
            }
        }

        let mut variants = BTreeMap::new();
        for (hash, draft) in drafts {
            let (Some(base_version), Some(storage), Some(creator), Some(message)) = (
                draft.base_version,
                draft.storage,
                draft.creator,
                draft.message,
            ) else {
                return Err(InverseIndexError::Malformed);
            };

            variants.insert(
                hash,
                VariantRecord {
                    base_version,
                    join: draft.join,
                    storage,
                    creator,
                    message,
                },
            );
        }

        let mut versions_by_variant: BTreeMap<Hash, Vec<Hash>> = BTreeMap::new();
        for (hash, record) in &versions {
            versions_by_variant
                .entry(record.variant)
                .or_default()
                .push(*hash);
        }

        let mut variants_by_base: BTreeMap<Hash, Vec<Hash>> = BTreeMap::new();
        let mut variants_by_join: BTreeMap<Hash, Vec<Hash>> = BTreeMap::new();
        let mut variants_by_storage: BTreeMap<Hash, Vec<Hash>> = BTreeMap::new();
        for (hash, record) in &variants {
            variants_by_base
                .entry(record.base_version)
                .or_default()
                .push(*hash);
            variants_by_storage
                .entry(record.storage)
                .or_default()
                .push(*hash);

            if let Some(join) = record.join {
                variants_by_join.entry(join).or_default().push(*hash);
            }
        }

        Ok(Self {
            variants,
            versions,
            versions_by_variant,
            variants_by_base,
            variants_by_join,
            variants_by_storage,
        })
    }

    /// Every variant, in key order.
    pub fn variants(&self) -> impl Iterator<Item = (Hash, &VariantRecord)> {
        self.variants.iter().map(|(hash, record)| (*hash, record))
    }

    /// Every version, in key order.
    pub fn versions(&self) -> impl Iterator<Item = (Hash, &VersionRecord)> {
        self.versions.iter().map(|(hash, record)| (*hash, record))
    }

    /// What the variant named `key` is made of.
    #[must_use]
    pub fn variant(&self, key: Hash) -> Option<&VariantRecord> {
        self.variants.get(&key)
    }

    /// What the version named `key` fixes, and its number.
    #[must_use]
    pub fn version(&self, key: Hash) -> Option<&VersionRecord> {
        self.versions.get(&key)
    }

    /// The versions that fix the variant named `variant`, in key order.
    #[must_use]
    pub fn versions_pinning(&self, variant: Hash) -> &[Hash] {
        self.versions_by_variant
            .get(&variant)
            .map_or(&[], Vec::as_slice)
    }

    /// The variants based on the version named `version`, in key order.
    #[must_use]
    pub fn variants_based_on(&self, version: Hash) -> &[Hash] {
        self.variants_by_base
            .get(&version)
            .map_or(&[], Vec::as_slice)
    }

    /// The variants that merge in the variant named `variant`, in key order.
    #[must_use]
    pub fn variants_joining(&self, variant: Hash) -> &[Hash] {
        self.variants_by_join
            .get(&variant)
            .map_or(&[], Vec::as_slice)
    }

    /// The variants made of the content stored under `storage`, in key order.
    #[must_use]
    pub fn variants_with_storage(&self, storage: Hash) -> &[Hash] {
        self.variants_by_storage
            .get(&storage)
            .map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use rorolala_vcs::{Hash, ROOT_VERSION};

    use super::Snapshot;
    use crate::{Edge, Table};

    /// A table holding one chain: root → a variant → a version, and a second variant on the version.
    fn chain() -> (Table, [Hash; 6]) {
        let root = Hash::new([0x01; 32]);
        let root_variant = Hash::new([0x02; 32]);
        let first_variant = Hash::new([0x03; 32]);
        let first_version = Hash::new([0x04; 32]);
        let second_variant = Hash::new([0x05; 32]);
        let storage = Hash::new([0x06; 32]);
        let creator = Hash::new([0x07; 32]);
        let message = Hash::new([0x08; 32]);

        let mut table = Table::new(vec![]);
        table.set_number(root, ROOT_VERSION);
        table.set_number(first_version, 0);
        table.add(root_variant, root, Edge::Variant);
        table.add(storage, first_variant, Edge::Storage);
        table.add(root, first_variant, Edge::Base);
        table.add(creator, first_variant, Edge::Creator);
        table.add(message, first_variant, Edge::Message);
        table.add(first_variant, first_version, Edge::Variant);
        table.add(storage, second_variant, Edge::Storage);
        table.add(first_version, second_variant, Edge::Base);
        table.add(creator, second_variant, Edge::Creator);
        table.add(message, second_variant, Edge::Message);

        (
            table,
            [
                root,
                root_variant,
                first_variant,
                first_version,
                second_variant,
                storage,
            ],
        )
    }

    #[test]
    fn a_snapshot_inverts_every_record() {
        let (
            table,
            [
                root,
                root_variant,
                first_variant,
                first_version,
                second_variant,
                storage,
            ],
        ) = chain();
        let snapshot =
            Snapshot::from_table(&table).expect("the records are the ones this build writes");

        // Two variants and two versions were named, and the root's own variant is named but absent.
        assert_eq!(snapshot.variants().count(), 2);
        assert_eq!(snapshot.versions().count(), 2);

        let first = snapshot
            .variant(first_variant)
            .expect("the first variant is there");
        assert_eq!(first.base_version, root);
        assert_eq!(first.join, None);
        assert_eq!(first.storage, storage);

        // The root fixes a variant the records name but nothing was written under.
        let root_record = snapshot.version(root).expect("the root is there");
        assert!(root_record.is_root);
        assert_eq!(root_record.number, None);
        assert_eq!(root_record.variant, root_variant);

        let first_record = snapshot
            .version(first_version)
            .expect("the first version is there");
        assert!(!first_record.is_root);
        assert_eq!(first_record.number, Some(0));
        assert_eq!(first_record.variant, first_variant);

        assert_eq!(snapshot.versions_pinning(first_variant), [first_version]);
        assert_eq!(snapshot.variants_based_on(first_version), [second_variant]);
        assert_eq!(
            snapshot.variants_with_storage(storage),
            [first_variant, second_variant]
        );
        let none: &[Hash] = &[];
        assert_eq!(snapshot.variants_joining(first_variant), none);
    }

    #[test]
    fn a_variant_without_every_part_is_not_a_record() {
        // A variant edge without the rest is not something this build writes: it is refused rather
        // than handed on as a variant with no content.
        let mut table = Table::new(vec![]);
        table.add(Hash::new([0x01; 32]), Hash::new([0x02; 32]), Edge::Storage);

        assert!(Snapshot::from_table(&table).is_err());
    }
}
