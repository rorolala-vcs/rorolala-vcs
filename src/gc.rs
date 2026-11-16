//! What a collecting run keeps: the closure the roots name.
//!
//! A collecting run's roots are a Layout's entries, and what they keep is everything the versions
//! they name reach: the version chain back to the root version, the variants it walks through and
//! the ones joined into it, the creator and message each variant carries, and the content those
//! variants store, a manifest and its chunks included. A tree version's content is its structure,
//! so the roots reach what the structure names too, the description and every file's content.
//!
//! Everything the roots do not reach is what a collecting run takes away; this module answers only
//! what they do reach, and says why it could not answer at all when an object the roots name is
//! missing or does not read. A run that cannot see the whole of what is wanted may not take
//! anything away, which is why a failure here stops the collecting rather than narrowing it.

use std::collections::{BTreeSet, HashSet};

use rorolala_errors::Failure as _;
use rorolala_layout::{EntryKind, Layout};
use rorolala_storage::{Blake3Hash, Key, RorolalaStorage};
use rorolala_tree::TreeStructure;
use rorolala_utils_progress::Progress;
use rorolala_vcs::{Hash, VCSIndex, VCSIndexObject};

/// What the roots keep, by the store each set is swept with.
#[derive(Clone, Debug, Default)]
pub struct Live {
    /// The index objects the roots reach, swept from the version control index.
    index: BTreeSet<Hash>,
    /// The content keys the roots reach, swept as object entries from the content store.
    objects: BTreeSet<Key>,
    /// The content keys the roots reach that carry a manifest, swept as manifest entries from the
    /// content store.
    manifests: BTreeSet<Key>,
}

impl Live {
    /// The index objects the roots reach.
    #[must_use]
    pub const fn index(&self) -> &BTreeSet<Hash> {
        &self.index
    }

    /// The content keys the roots reach, as object entries.
    #[must_use]
    pub const fn objects(&self) -> &BTreeSet<Key> {
        &self.objects
    }

    /// The content keys that carry a manifest the roots reach, as manifest entries.
    #[must_use]
    pub const fn manifests(&self) -> &BTreeSet<Key> {
        &self.manifests
    }
}

/// Why the closure could not be worked out, and so nothing may be taken away.
#[derive(Debug)]
pub enum CollectFailure {
    /// The index could not be read at all, so nothing about what the roots reach can be said.
    Index(String),
    /// The index holds nothing under a key the roots reach, or what it holds does not read.
    IndexObject {
        /// The key the roots reach that the index could not answer for.
        key: Hash,
        /// Why the index could not answer.
        reason: String,
    },
    /// A tree structure a live variant names could not be read back, or does not decode.
    ///
    /// A tree entry's storage key is its structure, and what the roots reach inside the tree is
    /// said by the structure and nothing else, so a structure this build cannot read is a closure
    /// that cannot be finished.
    TreeStructure {
        /// The key of the structure that could not be read.
        key: Key,
        /// Why the structure could not be read or decoded.
        reason: String,
    },
    /// The content store could not be read for a key the roots reach.
    Content {
        /// The key the store could not answer for.
        key: Key,
        /// Why the store could not answer.
        reason: String,
    },
}

impl std::fmt::Display for CollectFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Index(reason) => {
                write!(f, "the version control index could not be read: {reason}")
            }
            Self::IndexObject { key, reason } => {
                write!(
                    f,
                    "the index object {} could not be read: {reason}",
                    key.hex()
                )
            }
            Self::TreeStructure { key, reason } => write!(
                f,
                "the tree structure {} could not be read: {reason}",
                key.hex()
            ),
            Self::Content { key, reason } => {
                write!(
                    f,
                    "the stored content {} could not be read: {reason}",
                    key.hex()
                )
            }
        }
    }
}

/// Works out what the entries of `layout` keep, from the index and the content store they name.
///
/// The seeds are the entries' versions. Each one is read, and every object it names is read after
/// it: a version names its variant, a variant names the version it is based on, the variant it
/// joins when it merges one, its creator and its message, and the content it stores. Walking stops
/// at the root version, whose variant field is zeros and names no object; every chain ends there,
/// so the root version is among what the roots keep.
///
/// The content a variant stores is expanded by the kind of the entry the chain belongs to: a file
/// entry's storage key is the content, and a manifest under it names the chunks to keep with it; a
/// tree entry's storage key is its structure, and the structure names what is inside it, which is
/// expanded as file content. Two entries reaching one creator or one piece of content keep it
/// once, the sets are sets.
///
/// The work is said to `progress` as one task, advanced once per index object read; its length is
/// not known before the walk, since only the roots know how much of the index they reach.
///
/// # Errors
///
/// Returns [`CollectFailure`] when an object the roots reach cannot be read or does not read, and
/// so nothing may be taken away.
pub async fn live_of(
    layout: &Layout,
    index: &VCSIndex,
    store: &RorolalaStorage,
    progress: &Progress,
) -> Result<Live, CollectFailure> {
    // The listing is asked for once so an index that is failing is said before the walk starts:
    // a closure that cannot even be counted is one that cannot be finished, and the answer is the
    // same either way.
    index
        .object_keys()
        .await
        .map_err(|error| CollectFailure::Index(error.reason()))?;

    let mut live = Live::default();
    let mut seen: HashSet<Hash> = HashSet::new();
    let mut queue: Vec<(Hash, EntryKind)> = Vec::new();

    for (_, data) in layout.entries() {
        queue.push((Key::new(data.version()), data.kind()));
    }

    // The length of the walk is only known once it is over, so the task has no total to show and
    // is a marquee while it lasts.
    let mut task = progress.begin("", None);

    while let Some((key, kind)) = queue.pop() {
        if !seen.insert(key) {
            continue;
        }

        let object = index
            .read(key)
            .await
            .map_err(|error| CollectFailure::IndexObject {
                key,
                reason: error.reason(),
            })?;

        match object {
            VCSIndexObject::Version(version) => {
                live.index.insert(key);

                // The root version's variant field is zeros and names no object; every chain ends
                // there, and what a version that is not the root names is always a variant.
                let variant = version.variant();
                if *variant != [0_u8; 32] {
                    queue.push((Key::new(*variant), kind));
                }
            }
            VCSIndexObject::Variant(variant) => {
                live.index.insert(key);

                // The content the variant stores is kept for the entry the chain belongs to: a
                // file and a tree are told apart by the entry, whose kind does not change.
                expand(store, *variant.storage_hash(), kind, &mut live).await?;

                queue.push((Key::new(*variant.base_version()), kind));
                if let Some(join) = variant.join() {
                    queue.push((Key::new(*join), kind));
                }
                queue.push((Key::new(*variant.creator()), kind));
                queue.push((Key::new(*variant.message()), kind));
            }
            // A creator and a message carry text and name nothing, so they are leaves.
            VCSIndexObject::Creator(_) | VCSIndexObject::Message(_) => {
                live.index.insert(key);
            }
        }

        task.advance_by(1);
    }
    task.finish();

    Ok(live)
}

/// Marks the content `hash` names live, expanded the way an entry of `kind` names it.
///
/// A file entry's storage key is the content itself: a manifest under the key names the chunks to
/// keep with it, and the object entry under the key is kept too, the two are separate entries a
/// store holds side by side. A tree entry's storage key is its structure, written whole, and what
/// the structure names inside it, the description and every file's content, is kept the way file
/// content is.
///
/// A chunk is not expanded further: a chunk is written as an object, and a manifest under a chunk
/// key would be a manifest for content nobody references here, which stays dead unless one of the
/// roots names that key itself.
async fn expand(
    store: &RorolalaStorage,
    hash: Blake3Hash,
    kind: EntryKind,
    live: &mut Live,
) -> Result<(), CollectFailure> {
    let key = Key::new(hash);
    live.objects.insert(key);

    match kind {
        EntryKind::Tree => {
            let bytes =
                store
                    .read_object(&key)
                    .await
                    .map_err(|error| CollectFailure::TreeStructure {
                        key,
                        reason: error.to_string(),
                    })?;
            let structure =
                TreeStructure::decode(&bytes).map_err(|error| CollectFailure::TreeStructure {
                    key,
                    reason: error.to_string(),
                })?;

            for inside in structure.content_keys() {
                // A tree structure is flat, its entries name files and nothing deeper, so the walk
                // here is one level; the pin is what a recursive async function is written with.
                Box::pin(expand(store, *inside.digest(), EntryKind::File, live)).await?;
            }
        }
        EntryKind::File => {
            let manifest =
                store
                    .manifest_of(&key)
                    .await
                    .map_err(|error| CollectFailure::Content {
                        key,
                        reason: error.to_string(),
                    })?;

            if let Some(manifest) = manifest {
                live.manifests.insert(key);
                for chunk in manifest.chunks() {
                    live.objects.insert(chunk.key());
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rorolala_layout::{Layout, MutableData};
    use rorolala_storage::{Chunk, Codec, Manifest, RorolalaStorage};
    use rorolala_tree::{FileMode, TreeEntry, TreeStructure};
    use rorolala_vcs::{Creator, Message, VCSIndex, VCSWrite as _, Variant, Version};
    use uuid::Uuid;

    use super::{CollectFailure, EntryKind, Key, Progress, live_of};

    /// A directory of its own, emptied first so a rerun starts clean.
    fn scratch(label: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "rorolala-gc-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// The three sides one run works on, each in a directory of its own.
    fn sides(label: &str) -> (PathBuf, Layout, VCSIndex, RorolalaStorage) {
        let parent = scratch(label);
        let layout = Layout::open(parent.join("layout")).unwrap();
        let index = VCSIndex::create(parent.join("index"));
        let store = RorolalaStorage::create(parent.join("store"));

        (parent, layout, index, store)
    }

    /// A version that is the root: its variant field is zeros, and every chain ends here.
    fn root_version() -> Version {
        Version::new_bare_version([0_u8; 32], u64::MAX)
    }

    /// A progressive variant that stores `storage`, is based on `base`, and says who and what.
    fn variant_storing(storage: &Key, base: &Version, who: &Creator, what: &Message) -> Variant {
        Variant::new_bare_variant(
            *storage.digest(),
            *base.hash().digest(),
            None,
            *who.hash().digest(),
            *what.hash().digest(),
            0,
        )
    }

    /// The words one variant carries, written into `index`, as the pair the closures read back.
    async fn words(index: &VCSIndex) -> (Creator, Message) {
        let who = Creator::try_from(b"who".to_vec()).unwrap();
        let what = Message::try_from(b"what".to_vec()).unwrap();

        index.write(who.clone()).await.unwrap();
        index.write(what.clone()).await.unwrap();

        (who, what)
    }

    /// Lays an entry of `kind` at `version` down in `layout`.
    fn lay_entry(layout: &Layout, version: &Version, kind: EntryKind) {
        let data = MutableData::new(None, *version.hash().digest(), String::new()).with_kind(kind);

        let mut batch = layout.batch();
        batch.create_entry(Uuid::new_v4(), data).unwrap();
        batch.commit().unwrap();
    }

    /// A whole object of `content`, stored, as the key it is named by.
    async fn whole(store: &RorolalaStorage, content: &[u8]) -> Key {
        store.write_object(content, Codec::Raw).await.unwrap()
    }

    #[tokio::test]
    async fn an_entry_keeps_its_chain_its_words_and_its_content() {
        let (parent, layout, index, store) = sides("chain");
        let (who, what) = words(&index).await;
        let content_a = whole(&store, b"content a").await;
        let content_b = whole(&store, b"content b").await;

        let root = root_version();
        let first = variant_storing(&content_a, &root, &who, &what);
        let v1 = first.new_version();
        let second = variant_storing(&content_b, &v1, &who, &what);
        let v2 = second.new_version();

        // A variant nobody names: written here, it is a candidate edit, not a chain.
        let unjoined = Variant::new_bare_variant(
            [99_u8; 32],
            *v2.hash().digest(),
            None,
            *who.hash().digest(),
            *what.hash().digest(),
            0,
        );

        index.write(root.clone()).await.unwrap();
        index.write(first.clone()).await.unwrap();
        index.write(v1.clone()).await.unwrap();
        index.write(second.clone()).await.unwrap();
        index.write(v2.clone()).await.unwrap();
        index.write(unjoined.clone()).await.unwrap();
        lay_entry(&layout, &v2, EntryKind::File);

        let live = live_of(&layout, &index, &store, &Progress::silent())
            .await
            .unwrap();

        // The chain from the entry back to the root, the words it carries, and nothing else: the
        // unjoined variant is not on the chain, so nothing keeps it.
        let kept: BTreeSet<Key> = [
            root.hash(),
            first.hash(),
            v1.hash(),
            second.hash(),
            v2.hash(),
            who.hash(),
            what.hash(),
        ]
        .into_iter()
        .collect();
        assert_eq!(*live.index(), kept);
        assert!(!live.index().contains(&unjoined.hash()));

        // The content the chain stores, whole here, so no manifest entries are kept.
        assert_eq!(*live.objects(), BTreeSet::from([content_a, content_b]));
        assert_eq!(*live.manifests(), BTreeSet::new());

        let _ = fs::remove_dir_all(&parent);
    }

    #[tokio::test]
    async fn a_merge_keeps_the_variant_joined_in_and_what_it_stores() {
        let (parent, layout, index, store) = sides("merge");
        let (who, what) = words(&index).await;
        let content_a = whole(&store, b"content a").await;
        let content_j = whole(&store, b"content j").await;
        let content_m = whole(&store, b"content m").await;

        let root = root_version();
        let first = variant_storing(&content_a, &root, &who, &what);
        let v1 = first.new_version();

        // The variant joined in is based on v1 like the one that joins it, and what it stores is
        // kept: the merge reads it back when it is walked.
        let joined = variant_storing(&content_j, &v1, &who, &what);
        let merge = first.new_merge_variant(*content_m.digest(), &joined);
        let v2 = merge.new_version();

        index.write(root.clone()).await.unwrap();
        index.write(first.clone()).await.unwrap();
        index.write(v1.clone()).await.unwrap();
        index.write(joined.clone()).await.unwrap();
        index.write(merge.clone()).await.unwrap();
        index.write(v2.clone()).await.unwrap();
        lay_entry(&layout, &v2, EntryKind::File);

        let live = live_of(&layout, &index, &store, &Progress::silent())
            .await
            .unwrap();

        assert!(live.index().contains(&joined.hash()));
        assert!(live.index().contains(&merge.hash()));
        assert_eq!(
            *live.objects(),
            BTreeSet::from([content_a, content_j, content_m])
        );

        let _ = fs::remove_dir_all(&parent);
    }

    #[tokio::test]
    async fn a_tree_entry_keeps_its_structure_and_what_the_structure_names() {
        let (parent, layout, index, store) = sides("tree");
        let (who, what) = words(&index).await;
        let description = whole(&store, b"what the tree says").await;
        let file_a = whole(&store, b"file a").await;
        let file_b = whole(&store, b"file b").await;

        let structure = TreeStructure::build(
            Some(description),
            [
                TreeEntry::file("a", FileMode::Regular, file_a),
                TreeEntry::directory("dir"),
                TreeEntry::file("dir/b", FileMode::Regular, file_b),
            ],
        )
        .unwrap();
        let structure_key = store
            .write_object(&structure.encode(), Codec::Raw)
            .await
            .unwrap();

        let root = root_version();
        let variant = variant_storing(&structure_key, &root, &who, &what);
        let v1 = variant.new_version();

        index.write(root).await.unwrap();
        index.write(variant).await.unwrap();
        index.write(v1.clone()).await.unwrap();
        lay_entry(&layout, &v1, EntryKind::Tree);

        let live = live_of(&layout, &index, &store, &Progress::silent())
            .await
            .unwrap();

        // The structure is the entry's content, and what it names inside, the description and
        // every file's content, is kept with it; a directory names no content.
        assert_eq!(
            *live.objects(),
            BTreeSet::from([structure_key, description, file_a, file_b])
        );
        assert_eq!(*live.manifests(), BTreeSet::new());

        let _ = fs::remove_dir_all(&parent);
    }

    #[tokio::test]
    async fn a_file_kept_as_chunks_keeps_its_manifest_and_its_chunks() {
        let (parent, layout, index, store) = sides("chunks");
        let (who, what) = words(&index).await;
        let first_chunk = whole(&store, b"first chunk").await;
        let second_chunk = whole(&store, b"second chunk").await;

        // The content is chunked, so a manifest stands under its key and the chunks are what
        // holds it: the manifest entry and the chunk entries are kept with the content key. The
        // content key is arbitrary here, the closure marks keys and verifies none.
        let content = Key::new([7_u8; 32]);
        store
            .put_manifest(
                &content,
                &Manifest::new(vec![
                    Chunk::new(first_chunk, b"first chunk".len() as u64),
                    Chunk::new(second_chunk, b"second chunk".len() as u64),
                ]),
            )
            .await
            .unwrap();

        let root = root_version();
        let variant = variant_storing(&content, &root, &who, &what);
        let v1 = variant.new_version();

        index.write(root).await.unwrap();
        index.write(variant).await.unwrap();
        index.write(v1.clone()).await.unwrap();
        lay_entry(&layout, &v1, EntryKind::File);

        let live = live_of(&layout, &index, &store, &Progress::silent())
            .await
            .unwrap();

        assert_eq!(
            *live.objects(),
            BTreeSet::from([content, first_chunk, second_chunk])
        );
        assert_eq!(*live.manifests(), BTreeSet::from([content]));

        let _ = fs::remove_dir_all(&parent);
    }

    #[tokio::test]
    async fn a_root_the_index_cannot_answer_for_stops_the_closure() {
        let (parent, layout, index, store) = sides("missing");
        let (who, what) = words(&index).await;
        let content = whole(&store, b"content").await;

        let root = root_version();
        let variant = variant_storing(&content, &root, &who, &what);
        let v1 = variant.new_version();

        // The version the entry names is not written, so the closure cannot be finished: what
        // keeps what is unanswered, and nothing may be taken away.
        lay_entry(&layout, &v1, EntryKind::File);

        let failure = live_of(&layout, &index, &store, &Progress::silent())
            .await
            .unwrap_err();

        assert!(matches!(failure, CollectFailure::IndexObject { .. }));
        assert!(failure.to_string().contains(&v1.hash().hex()));

        let _ = fs::remove_dir_all(&parent);
    }
}
