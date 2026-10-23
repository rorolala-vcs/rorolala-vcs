//! What a store does when it is asked, asked of it through the real thing rather than beside it.
//!
//! The tests are in a file of their own so that the store is what the rest of the module is about;
//! they reach into it more deeply than a caller can, which is what a test of a layout has to do.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use rorolala_utils_constants::STORAGE_CONFIG_PATH;
use rorolala_utils_location::Locate as _;

use super::RorolalaStorage;
use super::consts::{MANIFEST_DIR, OBJECTS_DIR, PACKED_DIR, PackKind};
use super::entry::key_of;
use crate::{
    AlgorithmChoice, Chunk, Chunking, Codec, Error, FRAME_VERSION, Frame, Key, Layout, Manifest,
    StorageBackend as _, TransferableBackend as _, store_file, transfer,
};

/// A directory of its own, emptied first so a rerun starts clean.
fn scratch(label: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);

    let dir = std::env::temp_dir().join(format!(
        "rorolala-storage-{}-{label}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));

    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    dir
}

#[test]
fn making_a_store_lays_the_whole_of_it_down() {
    let parent = scratch("create");
    let root = parent.join("store");

    let store = RorolalaStorage::create(&root);

    // A store that exists is one with somewhere to put everything, so all three
    // directories are there from the start rather than as they fill up.
    assert!(root.join(STORAGE_CONFIG_PATH).is_file());
    assert!(root.join(OBJECTS_DIR).is_dir());
    assert!(root.join(MANIFEST_DIR).is_dir());
    assert!(root.join(PACKED_DIR).is_dir());
    assert_eq!(store.get_root(), root.as_path());
    assert_eq!(store.config_path(), root.join(STORAGE_CONFIG_PATH));

    let _ = fs::remove_dir_all(&parent);
}

#[test]
fn a_store_is_told_apart_by_its_configuration() {
    let parent = scratch("locate");
    let root = parent.join("store");

    // Nothing is a store until it is made into one.
    assert!(RorolalaStorage::at(&root).is_none());

    let _ = RorolalaStorage::create(&root);

    assert!(RorolalaStorage::at(&root).is_some());
    // And one is found from inside it as much as from its root.
    let inside = root.join(OBJECTS_DIR);
    assert_eq!(RorolalaStorage::locate(&inside).unwrap().get_root(), root);

    let _ = fs::remove_dir_all(&parent);
}

#[test]
fn a_key_names_where_its_object_sits() {
    let parent = scratch("paths");
    let root = parent.join("store");
    let store = RorolalaStorage::create(&root);

    let key = Key::new([0xAB; 32]);
    let hex = key.hex();
    let sharded = |directory: &str| {
        root.join(directory)
            .join(&hex[..2])
            .join(&hex[2..4])
            .join(&hex)
    };

    assert_eq!(store.object_path(&key), sharded(OBJECTS_DIR));
    assert_eq!(store.manifest_path(&key), sharded(MANIFEST_DIR));
    assert_eq!(
        store.pack_paths(7),
        (
            root.join(PACKED_DIR).join("packed_7.pack"),
            root.join(PACKED_DIR).join("packed_7.idx")
        )
    );

    let _ = fs::remove_dir_all(&parent);
}

/// A store of its own, in a directory of its own.
fn store(label: &str) -> (PathBuf, RorolalaStorage) {
    let parent = scratch(label);
    let storage = RorolalaStorage::create(parent.join("store"));

    (parent, storage)
}

/// Whether `store` can produce `key`, asked of it in a batch of one.
async fn holds(store: &RorolalaStorage, key: &Key) -> bool {
    store
        .contains_keys(&[*key])
        .await
        .expect("the store answers")
        .held(0)
}

/// Lays `manifest` down under `key` straight through the filesystem.
///
/// What is being asked about in the tests that use this is a manifest that disagrees with the chunks
/// it names, which a store gone wrong has and a store's own write never makes. Going through the
/// filesystem is what leaves it there.
fn lay_manifest(store: &RorolalaStorage, key: &Key, manifest: &Manifest) {
    let plain = manifest.encode();
    let frame = Frame {
        version: FRAME_VERSION,
        codec: Codec::Raw,
        layout: Layout::Chunked,
        plain_len: plain.len() as u64,
    };
    let mut bytes = frame.encode();
    bytes.extend_from_slice(&plain);

    let path = store.manifest_path(key);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

#[tokio::test]
async fn content_written_is_content_read_back() {
    let (parent, store) = store("round-trip");
    let file = parent.join("file");
    let content = b"the same bytes, however they were kept";
    fs::write(&file, content).unwrap();

    let key = store_file(&store, &file).await.unwrap();

    // The key is the hash of what was written, so it can be checked against the content
    // rather than taken on trust.
    assert_eq!(key, Key::new(*blake3::hash(content).as_bytes()));
    assert!(holds(&store, &key).await);

    let out = parent.join("out");
    store.extract_file(&key, &out).await.unwrap();
    assert_eq!(fs::read(&out).unwrap(), content);

    // A key nothing is stored under is neither found nor claimed to be held.
    let absent = Key::new([0; 32]);
    assert!(!holds(&store, &absent).await);
    assert!(matches!(
        store.extract_file(&absent, &out).await,
        Err(Error::NotFound(_))
    ));

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn a_cut_content_is_read_back_whole() {
    let (parent, store) = store("chunked");
    let content: Vec<u8> = (0..1000_u32).map(|value| (value % 251) as u8).collect();
    let file = parent.join("file");
    fs::write(&file, &content).unwrap();

    let choice = AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 64 });
    let key = store.write_file(&file, choice).await.unwrap();

    // Cut content is put back together from a manifest rather than stored whole.
    let manifest = store.read_manifest(&key).await.unwrap().unwrap();
    assert!(manifest.len() >= 2);
    assert!(store.manifest_path(&key).is_file());
    assert!(holds(&store, &key).await);

    let out = parent.join("out");
    store.extract_file(&key, &out).await.unwrap();
    assert_eq!(fs::read(&out).unwrap(), content);

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn a_cut_content_is_told_from_a_whole_one() {
    let (parent, store) = store("manifests");

    let whole = store.write_object(b"kept whole", Codec::Raw).await.unwrap();
    let content: Vec<u8> = (0..1000_u32).map(|value| (value % 251) as u8).collect();
    let file = parent.join("file");
    fs::write(&file, &content).unwrap();
    let cut = store
        .write_file(
            &file,
            AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 64 }),
        )
        .await
        .unwrap();

    // Which of the two a key is kept as is what tells them apart.
    assert!(!store.holds_manifest(&whole).await.unwrap());
    assert!(store.holds_manifest(&cut).await.unwrap());

    // And it is only the cut one that a listing of manifests names.
    assert_eq!(store.list_manifest_keys().await.unwrap(), [cut]);

    // Writing the same content back whole drops the manifest: an object is the entry then, so what
    // the key is kept as follows the last write rather than being left over from the one before.
    let again = store
        .write_file(&file, AlgorithmChoice::new(Codec::Raw, Chunking::Whole))
        .await
        .unwrap();
    assert_eq!(again, cut);
    assert!(!store.holds_manifest(&cut).await.unwrap());
    assert_eq!(store.list_manifest_keys().await.unwrap(), Vec::new());

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn what_a_key_is_carried_as_is_how_long_its_content_is() {
    let (parent, store) = store("carried-len");

    // A content kept whole is carried as the object itself, so it is as long as what was written.
    let content = b"the same bytes, however they were kept";
    let whole = store.write_object(content, Codec::Raw).await.unwrap();
    assert_eq!(
        store.carried_len(&whole).await.unwrap(),
        content.len() as u64
    );

    // A content kept as chunks is carried as its manifest and then the chunks it names, so it is as
    // long as the content plus the manifest that says how it was cut.
    let cut_content: Vec<u8> = (0..1000_u32).map(|value| (value % 251) as u8).collect();
    let file = parent.join("file");
    fs::write(&file, &cut_content).unwrap();
    let cut = store
        .write_file(
            &file,
            AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 64 }),
        )
        .await
        .unwrap();
    let manifest = store.read_manifest(&cut).await.unwrap().unwrap();
    assert_eq!(
        store.carried_len(&cut).await.unwrap(),
        cut_content.len() as u64 + manifest.encode().len() as u64
    );

    // A key nothing is stored under is not one that can be measured.
    assert!(matches!(
        store.carried_len(&Key::new([0; 32])).await,
        Err(Error::NotFound(_))
    ));

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn what_a_key_is_carried_as_is_not_how_it_is_kept() {
    let (parent, storage) = store("carried-len-codec");
    let store = storage.with_codec(Codec::Zstd);

    // Repetition compresses, but what crosses is the content rather than the encoding of it, so a
    // key is carried as long as its content however the store chose to keep it.
    let content = vec![0_u8; 4096];
    let key = store.write_object(&content, Codec::Zstd).await.unwrap();

    assert_eq!(store.carried_len(&key).await.unwrap(), content.len() as u64);

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn what_is_already_held_is_not_written_again() {
    let (parent, store) = store("rewrite");
    let content: Vec<u8> = (0..8 * 1024_u32).map(|value| (value % 251) as u8).collect();
    let file = parent.join("file");
    fs::write(&file, &content).unwrap();

    let choice = AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 1024 });
    let key = store.write_file(&file, choice).await.unwrap();
    let chunks: Vec<Key> = store
        .read_manifest(&key)
        .await
        .unwrap()
        .unwrap()
        .chunks()
        .iter()
        .map(Chunk::key)
        .collect();
    assert!(chunks.len() >= 2);

    // The chunks are packed, so the whole of what this content is kept as is already held.
    store.pack(&chunks).await.unwrap();
    for chunk in &chunks {
        assert!(!store.object_path(chunk).is_file());
    }

    // Writing the same content again is writing nothing: a chunk that is held — packed as much as
    // loose — is not written a second time. Without that, this would leave a loose copy of every
    // packed chunk behind, which is the same bytes in two places.
    let again = store.write_file(&file, choice).await.unwrap();
    assert_eq!(again, key);

    for chunk in &chunks {
        assert!(
            !store.object_path(chunk).is_file(),
            "a packed chunk was written loose again"
        );
    }

    // And what is held is still the content, read back whole.
    let out = parent.join("out");
    store.extract_file(&key, &out).await.unwrap();
    assert_eq!(fs::read(&out).unwrap(), content);

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn a_manifest_is_packed_among_the_manifests() {
    let (parent, store) = store("manifest-pack");
    let content: Vec<u8> = (0..4 * 1024_u32).map(|value| (value % 251) as u8).collect();
    let file = parent.join("file");
    fs::write(&file, &content).unwrap();

    let key = store
        .write_file(
            &file,
            AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 1024 }),
        )
        .await
        .unwrap();
    assert!(store.manifest_path(&key).is_file());

    // A manifest is packed like anything else, but into a pack of its own kind, and that pack keeps
    // its index among the manifests rather than beside the packs.
    let made = store.pack(&[key]).await.unwrap();
    assert!(made, "the manifest was not packed");

    assert!(!store.manifest_path(&key).is_file());
    assert!(store.index_path(PackKind::Manifest, 0).is_file());
    assert!(!store.index_path(PackKind::Object, 0).exists());
    assert!(store.pack_path(0).is_file());

    // The manifest still reads, is still listed, and still puts the content back together.
    assert!(store.read_manifest(&key).await.unwrap().is_some());
    assert_eq!(store.list_manifest_keys().await.unwrap(), [key]);

    let out = parent.join("out");
    store.extract_file(&key, &out).await.unwrap();
    assert_eq!(fs::read(&out).unwrap(), content);

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn a_manifest_can_be_carried_to_another_store() {
    let (parent, from) = store("carry-from");
    let to = RorolalaStorage::create(parent.join("to"));

    let content: Vec<u8> = (0..4 * 1024_u32).map(|value| (value % 251) as u8).collect();
    let file = parent.join("file");
    fs::write(&file, &content).unwrap();
    let key = from
        .write_file(
            &file,
            AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 1024 }),
        )
        .await
        .unwrap();

    // What a caller has to hand on is the manifest and the chunks it names, and nothing else — the
    // content is put back together on the other side from those.
    let manifest = from.manifest_of(&key).await.unwrap().unwrap();
    to.put_manifest(&key, &manifest).await.unwrap();

    for chunk in manifest.chunks() {
        let bytes = from.read_object(&chunk.key()).await.unwrap();
        to.write_object(&bytes, Codec::Raw).await.unwrap();
    }

    // The other store holds the content, and holds it cut the way it was told to be.
    assert!(to.holds_content(&key).await.unwrap());
    assert!(to.manifest_of(&key).await.unwrap().is_some());

    let out = parent.join("carried");
    to.extract_file(&key, &out).await.unwrap();
    assert_eq!(fs::read(&out).unwrap(), content);

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn what_is_stored_is_listed_and_can_be_dropped() {
    let (parent, store) = store("list");

    let one = store.write_object(b"one", Codec::Raw).await.unwrap();
    let two = store.write_object(b"two", Codec::Raw).await.unwrap();

    let mut wanted = vec![one, two];
    wanted.sort();
    assert_eq!(store.list_all_keys().await.unwrap(), wanted);

    store.remove(&one).await.unwrap();

    assert!(!holds(&store, &one).await);
    assert_eq!(store.list_all_keys().await.unwrap(), [two]);
    assert!(matches!(store.remove(&one).await, Err(Error::NotFound(_))));

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn removing_a_key_takes_away_what_is_loose_and_what_is_packed() {
    let (parent, store) = store("remove-both");
    let content = b"kept twice over";
    let key = store.write_object(content, Codec::Raw).await.unwrap();
    let entry = fs::read(store.object_path(&key)).unwrap();

    // Packed, and then loose again: an old pack beside a later write leaves one key in both places,
    // and a removal that stopped at the first would leave the content readable.
    assert!(store.pack(&[key]).await.unwrap());
    assert!(!store.object_path(&key).exists());
    fs::create_dir_all(store.object_path(&key).parent().unwrap()).unwrap();
    fs::write(store.object_path(&key), &entry).unwrap();

    store.remove(&key).await.unwrap();

    assert!(!store.holds_object(&key).await.unwrap());
    assert!(!store.object_path(&key).exists());
    assert_eq!(store.list_all_keys().await.unwrap(), Vec::<Key>::new());
    assert!(matches!(
        store.extract_file(&key, &parent.join("out")).await,
        Err(Error::NotFound(_))
    ));

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn a_key_a_pack_names_is_held_though_the_pack_file_is_gone() {
    let (parent, store) = store("pack-gone");
    let key = store
        .write_object(b"packed and then lost", Codec::Raw)
        .await
        .unwrap();
    assert!(store.pack(&[key]).await.unwrap());

    // The index is what a lookup reads, and it still names the key; the pack file is what a read
    // opens. Held is the index's answer, so the key is held and not producible — which is the case a
    // transfer answers with "nothing to send" rather than by failing.
    let (pack, index) = store.pack_paths(0);
    fs::remove_file(&pack).unwrap();
    assert!(index.is_file());

    assert!(holds(&store, &key).await);
    assert_eq!(store.list_exist_keys().await.unwrap(), [key]);
    assert!(matches!(store.content(&key).await, Ok(None)));

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn content_written_in_any_codec_and_cut_comes_back_the_same() {
    let (parent, store) = store("codecs");
    let content = "the same words over and over, ".repeat(64).into_bytes();
    let file = parent.join("file");
    fs::write(&file, &content).unwrap();

    let mut written = 0;
    for codec in [Codec::Raw, Codec::Lz4, Codec::Zstd] {
        for chunking in [Chunking::Whole, Chunking::Fixed { size: 256 }] {
            let key = store
                .write_file(&file, AlgorithmChoice::new(codec, chunking))
                .await
                .unwrap();

            // The key is the hash of the content, whatever it was encoded or cut with.
            assert_eq!(key, Key::new(*blake3::hash(&content).as_bytes()));

            let out = parent.join(format!("out-{written}"));
            store.extract_file(&key, &out).await.unwrap();
            assert_eq!(fs::read(&out).unwrap(), content, "{codec:?} {chunking:?}");

            written += 1;
        }
    }

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn a_chunk_that_is_not_the_length_the_manifest_says_is_refused() {
    let (parent, store) = store("chunk-len");
    let content: Vec<u8> = (0..1000_u32).map(|value| (value % 251) as u8).collect();
    let file = parent.join("file");
    fs::write(&file, &content).unwrap();

    let key = store
        .write_file(
            &file,
            AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 64 }),
        )
        .await
        .unwrap();

    // The same chunks, with the first said to be a byte longer than it is.
    let manifest = store.read_manifest(&key).await.unwrap().unwrap();
    let mut tampered = Manifest::default();
    for (at, held) in manifest.chunks().iter().enumerate() {
        let len = if at == 0 { held.len() + 1 } else { held.len() };
        tampered.push(Chunk::new(held.key(), len));
    }
    // Written straight down rather than through the store: what is being asked about is a manifest on
    // the disk that disagrees with the chunks it names, which is what a store gone wrong has, and the
    // store writes a manifest that reads.
    let plain = tampered.encode();
    let frame = Frame {
        version: FRAME_VERSION,
        codec: Codec::Raw,
        layout: Layout::Chunked,
        plain_len: plain.len() as u64,
    };
    let mut bytes = frame.encode();
    bytes.extend_from_slice(&plain);
    fs::write(store.manifest_path(&key), bytes).unwrap();

    assert!(matches!(
        store.extract_file(&key, &parent.join("out")).await,
        Err(Error::Malformed)
    ));

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn a_manifest_that_does_not_read_does_not_hide_the_object() {
    let (parent, store) = store("bad-manifest");
    let content = b"the content, kept whole";
    let key = store.write_object(content, Codec::Raw).await.unwrap();

    // A stray file where a manifest would be, which does not read as one: the object beside it
    // is still the content the key names, so the key is not lost to it.
    let manifest = store.manifest_path(&key);
    fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    fs::write(&manifest, b"not a manifest").unwrap();

    let out = parent.join("out");
    store.extract_file(&key, &out).await.unwrap();
    assert_eq!(fs::read(&out).unwrap(), content);
    // What the store says it holds has to agree with what it hands back.
    assert!(holds(&store, &key).await);

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn a_manifest_whose_chunks_are_gone_does_not_hide_the_object() {
    let (parent, store) = store("gone-chunks");
    let content: Vec<u8> = (0..1000_u32).map(|value| (value % 251) as u8).collect();
    let file = parent.join("file");
    fs::write(&file, &content).unwrap();

    let key = store
        .write_file(
            &file,
            AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 64 }),
        )
        .await
        .unwrap();

    // The whole content is under the key as well — a write of it lands at the same key — and a
    // chunk of the manifest is then taken away, so the manifest is one nothing can be put back
    // together from while the content itself is still there.
    store.write_object(&content, Codec::Raw).await.unwrap();
    let manifest = store.read_manifest(&key).await.unwrap().unwrap();
    store.remove(&manifest.chunks()[0].key()).await.unwrap();

    let out = parent.join("out");
    store.extract_file(&key, &out).await.unwrap();
    assert_eq!(fs::read(&out).unwrap(), content);
    assert!(holds(&store, &key).await);

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn temporaries_left_by_a_write_are_cleared() {
    let (parent, store) = store("temporaries");
    let key = store.write_object(b"the object", Codec::Raw).await.unwrap();

    // A temporary beside the object, as a write cut short would leave.
    let stray = store.object_path(&key).with_file_name("stray.1.2.tmp");
    fs::write(&stray, b"half a write").unwrap();

    assert_eq!(store.clear_temporaries().await.unwrap(), 1);
    assert!(!stray.exists());
    // What is not a temporary is not swept: the object is still held.
    assert!(holds(&store, &key).await);

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn content_that_does_not_hash_to_its_key_is_refused() {
    let (parent, store) = store("corrupt");

    let key = store
        .write_object(b"the real content", Codec::Raw)
        .await
        .unwrap();

    // Something else, laid where the key says and framed as if it were the right thing.
    let frame = Frame {
        version: FRAME_VERSION,
        codec: Codec::Raw,
        layout: Layout::Single,
        plain_len: 5,
    };
    let mut bytes = frame.encode();
    bytes.extend_from_slice(b"other");
    fs::write(store.object_path(&key), bytes).unwrap();

    assert!(matches!(
        store.read_object(&key).await,
        Err(Error::Corrupt(_))
    ));

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn an_object_that_is_there_is_taken_as_whole_and_not_written_again() {
    let (parent, store) = store("trust-exists");
    let content = b"the content that was written";
    let key = store.write_object(content, Codec::Raw).await.unwrap();

    // Something else laid where the key says and framed as an object. A write takes existence for
    // completeness: repairing it here would mean reading and hashing every object the store already
    // holds, on every write, so the damage is what the read reports instead.
    let frame = Frame {
        version: FRAME_VERSION,
        codec: Codec::Raw,
        layout: Layout::Single,
        plain_len: 5,
    };
    let mut bytes = frame.encode();
    bytes.extend_from_slice(b"other");
    fs::write(store.object_path(&key), bytes).unwrap();

    assert_eq!(store.write_object(content, Codec::Raw).await.unwrap(), key);
    assert!(matches!(
        store.read_object(&key).await,
        Err(Error::Corrupt(_))
    ));

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn a_directory_where_an_object_belongs_is_not_an_object() {
    let (parent, store) = store("object-dir");
    let content = b"the content a directory is not";
    let key = Key::new(*blake3::hash(content).as_bytes());

    // Something that is not a file where the object belongs. It is not held, so the write goes
    // ahead rather than answering `Ok` over something no read could ever open — and it fails,
    // because a file cannot be moved over a directory.
    fs::create_dir_all(store.object_path(&key)).unwrap();
    assert!(!holds(&store, &key).await);

    assert!(matches!(
        store.write_object(content, Codec::Raw).await,
        Err(Error::Io(_))
    ));

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn the_choice_is_a_container_taken_apart_or_the_content_itself() {
    let (parent, store) = store("choice");

    // Text is compressed, and cut once there is enough of it to be worth a manifest.
    let short = parent.join("short.txt");
    fs::write(&short, b"a line\nanother line\n").unwrap();
    let choice = store.choose_algorithm(&short);
    assert_eq!(choice.codec(), Codec::Zstd);
    assert_eq!(choice.chunking(), Chunking::Whole);

    let long = parent.join("long.txt");
    let mut lines = String::new();
    for line in 0..2000 {
        let _ = writeln!(lines, "line {line}");
    }
    fs::write(&long, &lines).unwrap();
    let choice = store.choose_algorithm(&long);
    assert_eq!(choice.codec(), Codec::Zstd);
    assert!(
        matches!(choice.chunking(), Chunking::Text { .. }),
        "{:?}",
        choice.chunking()
    );

    // Content that is not text and is not a container is written as it came in, whole: neither
    // compressing nor cutting is done unasked.
    let binary = parent.join("binary");
    fs::write(&binary, vec![0_u8; 1 << 20]).unwrap();
    let choice = store.choose_algorithm(&binary);
    assert_eq!(choice.codec(), Codec::Raw);
    assert_eq!(choice.chunking(), Chunking::Whole);

    // A packed container is taken apart at its members wherever it is, however small it is,
    // and is not compressed: what it holds is compressed already.
    let packed = parent.join("deck.pptx");
    fs::write(&packed, b"PK\x03\x04 and the rest of a container").unwrap();
    let choice = store.choose_algorithm(&packed);
    assert_eq!(choice.codec(), Codec::Raw);
    assert_eq!(choice.chunking(), Chunking::Zip);

    // What a store was told to write is what it writes, and what it was told applies to content of
    // any kind: a signature or a policy's claim is not allowed to overrule a caller who named the
    // strategy. What it does *not* change is how the content is encoded — a cut is a cut, and text
    // is still text.
    let told = store
        .clone()
        .with_codec(Codec::Lz4)
        .with_forced_cut(Chunking::Cdc {
            min: 64 * 1024,
            average: 256 * 1024,
            max: 1024 * 1024,
        });

    let choice = told.choose_algorithm(&binary);
    assert_eq!(choice.codec(), Codec::Lz4);
    assert_eq!(
        choice.chunking(),
        Chunking::Cdc {
            min: 64 * 1024,
            average: 256 * 1024,
            max: 1024 * 1024,
        }
    );

    for file in [&packed, &long] {
        let choice = told.choose_algorithm(file);

        // The container is still stored as it came in and the text is still compressed: forcing the
        // cut decided how the pieces are found and nothing else.
        assert_eq!(choice.codec(), store.choose_algorithm(file).codec());
        assert_eq!(
            choice.chunking(),
            Chunking::Cdc {
                min: 64 * 1024,
                average: 256 * 1024,
                max: 1024 * 1024,
            }
        );
    }

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn a_manifest_that_puts_the_chunks_in_the_wrong_order_is_not_the_content() {
    let (parent, store) = store("wrong-order");
    let content: Vec<u8> = (0..1000_u32).map(|value| (value % 251) as u8).collect();
    let file = parent.join("file");
    fs::write(&file, &content).unwrap();

    let key = store
        .write_file(
            &file,
            AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 64 }),
        )
        .await
        .unwrap();
    let manifest = store.read_manifest(&key).await.unwrap().unwrap();
    assert!(manifest.len() >= 2);

    // The same chunks, in the order that is not the content's. Every chunk is whole in itself, so
    // only a check of the whole can tell that the key does not name what they make.
    let mut shuffled = Manifest::default();
    for held in manifest.chunks().iter().rev() {
        shuffled.push(Chunk::new(held.key(), held.len()));
    }
    lay_manifest(&store, &key, &shuffled);

    assert!(matches!(
        store.content(&key).await,
        Err(Error::Corrupt(corrupt)) if corrupt == key
    ));

    let out = parent.join("out");
    assert!(matches!(
        store.extract_file(&key, &out).await,
        Err(Error::Corrupt(_))
    ));
    assert!(
        !out.exists(),
        "content that is not the key's was written out"
    );

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn a_manifest_that_makes_another_content_falls_back_to_the_object() {
    let (parent, store) = store("fallback-whole");
    let content = b"the content the key names, kept whole";
    let key = store.write_object(content, Codec::Raw).await.unwrap();

    // A manifest naming a chunk of another content of the same length: it puts something back
    // together, and what it puts back is not what the key promises. The object beside it is.
    let other = vec![b'x'; content.len()];
    let other_key = store.write_object(&other, Codec::Raw).await.unwrap();
    let mut manifest = Manifest::default();
    manifest.push(Chunk::new(other_key, other.len() as u64));
    lay_manifest(&store, &key, &manifest);

    assert_eq!(store.content(&key).await.unwrap().unwrap(), content);

    let out = parent.join("out");
    store.extract_file(&key, &out).await.unwrap();
    assert_eq!(fs::read(&out).unwrap(), content);

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn a_store_that_cannot_read_is_not_a_store_with_a_bad_manifest() {
    let (parent, store) = store("malformed-manifest-io");
    let key = Key::new([7; 32]);

    // A manifest that does not read, and something where the object would be that is not a file at
    // all: the object is the fallback, and what went wrong reading it is what a caller needs to be
    // told. Reporting "malformed" would send them looking at the manifest and not at the disk.
    let manifest = store.manifest_path(&key);
    fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    fs::write(&manifest, b"not a manifest").unwrap();
    fs::create_dir_all(store.object_path(&key)).unwrap();

    assert!(matches!(
        store.extract_file(&key, &parent.join("out")).await,
        Err(Error::Io(_))
    ));

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn a_store_that_cannot_read_the_whole_is_not_content_that_does_not_add_up() {
    let (parent, store) = store("assemble-io");
    let content: Vec<u8> = (0..1000_u32).map(|value| (value % 251) as u8).collect();
    let file = parent.join("file");
    fs::write(&file, &content).unwrap();

    let key = store
        .write_file(
            &file,
            AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 64 }),
        )
        .await
        .unwrap();

    // The chunks cannot be put back together, so the whole object is asked for — and what is at its
    // path is not an object that can be read. The failure of that read is a disk's, not a manifest's.
    let manifest = store.read_manifest(&key).await.unwrap().unwrap();
    store.remove(&manifest.chunks()[0].key()).await.unwrap();
    fs::create_dir_all(store.object_path(&key)).unwrap();

    assert!(matches!(
        store.extract_file(&key, &parent.join("out")).await,
        Err(Error::Io(_))
    ));

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn extracting_makes_the_directory_the_path_is_in() {
    let (parent, store) = store("extract-dir");
    let content = b"written somewhere new";
    let key = store.write_object(content, Codec::Raw).await.unwrap();

    // A caller names a path, not a path whose way has been made: the extract makes it, and what
    // lands there is whole.
    let out = parent.join("deeper").join("still").join("out");
    store.extract_file(&key, &out).await.unwrap();
    assert_eq!(fs::read(&out).unwrap(), content);

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn content_larger_than_one_write_comes_back_whole() {
    let (parent, store) = store("write-complete");

    // Larger than the stretch tokio hands its blocking writer at once, so the entry is written in
    // several writes: what a read gets back is the whole of it, frame and all. The flush in
    // `write_entry` is what keeps the last of those writes from being in flight when the temporary
    // is moved into place; a race that breaks it is not one a test can ask for on purpose, so this
    // is the round trip rather than the race.
    let content: Vec<u8> = (0..5 * 1024 * 1024_u32)
        .map(|value| (value % 251) as u8)
        .collect();
    let key = store.write_object(&content, Codec::Raw).await.unwrap();

    assert_eq!(store.read_object(&key).await.unwrap(), content);

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn an_extract_that_cannot_land_leaves_no_temporary() {
    let (parent, store) = store("extract-failed");
    let key = store
        .write_object(b"content that never lands", Codec::Raw)
        .await
        .unwrap();

    // Something where the file is to go that the move cannot land on: the target is left as it was,
    // and what was written beside it is taken back rather than left for a caller to find.
    let out = parent.join("out");
    fs::create_dir_all(&out).unwrap();

    assert!(matches!(
        store.extract_file(&key, &out).await,
        Err(Error::Io(_))
    ));
    assert!(out.is_dir());

    let strays = fs::read_dir(&parent)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .count();
    assert_eq!(strays, 0, "a failed extract left a temporary behind");

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn content_a_manifest_does_not_make_is_not_sent() {
    let (parent, from) = store("send-corrupt");
    let (_, to) = store("send-corrupt-to");

    let content: Vec<u8> = (0..1000_u32).map(|value| (value % 251) as u8).collect();
    let file = parent.join("file");
    fs::write(&file, &content).unwrap();
    let key = from
        .write_file(
            &file,
            AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 64 }),
        )
        .await
        .unwrap();

    // The chunks are the right ones, in the wrong order: the content the manifest makes is not the
    // content the key names, so there is nothing this end may put on the wire for it.
    let manifest = from.read_manifest(&key).await.unwrap().unwrap();
    let mut shuffled = Manifest::default();
    for held in manifest.chunks().iter().rev() {
        shuffled.push(Chunk::new(held.key(), held.len()));
    }
    lay_manifest(&from, &key, &shuffled);

    let (mut mine, mut theirs) = tokio::io::duplex(64 * 1024);
    let answering_with = to.clone();
    let answering =
        tokio::spawn(async move { transfer::respond(&answering_with, &mut theirs).await });

    let sent = transfer::initiate(&from, &mut mine, &[key]).await;

    // The exchange ends with the store's account of what is wrong, rather than reading the key as
    // "not there" or handing over bytes that are not what it names.
    assert!(matches!(
        sent,
        Err(transfer::Error::Store(Error::Corrupt(corrupt))) if corrupt == key
    ));

    // The peer is waiting for content that is not coming; closing this end tells it so.
    drop(mine);
    let _ = answering.await;

    assert!(!holds(&to, &key).await);

    let _ = fs::remove_dir_all(&parent);
}

#[tokio::test]
async fn a_key_a_pack_names_but_cannot_produce_does_not_stall_an_exchange() {
    let (parent, from) = store("send-gone-pack");
    let (_, to) = store("send-gone-pack-to");

    let key = from
        .write_object(b"packed and then gone", Codec::Raw)
        .await
        .unwrap();
    assert!(from.pack(&[key]).await.unwrap());

    // The index still names the key and the pack file is gone: this end answers that it holds it,
    // and cannot produce it. The exchange says "nothing here" for it and ends — the case the empty
    // frame in `send` exists for.
    let (pack, _) = from.pack_paths(0);
    fs::remove_file(&pack).unwrap();

    let (mut mine, mut theirs) = tokio::io::duplex(64 * 1024);
    let answering_with = to.clone();
    let answering =
        tokio::spawn(async move { transfer::respond(&answering_with, &mut theirs).await });

    transfer::initiate(&from, &mut mine, &[key]).await.unwrap();
    answering.await.unwrap().unwrap();

    assert!(holds(&from, &key).await, "held is the index's answer");
    assert!(!holds(&to, &key).await, "nothing crossed for it");

    let _ = fs::remove_dir_all(&parent);
}

/// What a batch writes is what the store reads back, and it is the store's from the moment it is
/// published rather than before.
#[tokio::test]
async fn a_batch_writes_a_store_that_reads_back() {
    let (parent, store) = store("batch");

    let one = b"the first thing".to_vec();
    let two = b"the second thing".to_vec();
    // A cut content keeps the whole of itself under its key as well as the manifest that says how it
    // was cut, so this is the one key that really is two entries: the content, and the way it is put
    // back together.
    let whole = b"the content that was cut".to_vec();
    let cut = key_of(&whole);
    let first = key_of(&one);
    let second = key_of(&two);

    let mut manifest = Manifest::default();
    manifest.push(Chunk::new(key_of(b"a chunk"), 7));

    let mut batch = store.begin_batch().await;
    batch.add(&first, &one).await.unwrap();
    batch.add(&second, &two).await.unwrap();
    // A key named twice in one run is one entry, since an index holds one of each.
    // One key, one object and one manifest: a chunk whose bytes are the very content some key names
    // is exactly this, and the two are entries of different kinds rather than one entry written
    // twice.
    batch.add(&cut, &whole).await.unwrap();
    batch.add_manifest(&cut, &manifest).await.unwrap();

    // A pack is found by its index, so what a batch has written is a pack no reader reaches until
    // the batch publishes it.
    assert!(!holds(&store, &second).await, "not a store's yet");

    assert_eq!(batch.publish().await.unwrap(), 4);

    assert_eq!(store.read_object(&first).await.unwrap(), one);
    assert_eq!(store.read_object(&second).await.unwrap(), two);
    assert_eq!(store.read_object(&cut).await.unwrap(), whole);
    assert_eq!(store.manifest_of(&cut).await.unwrap(), Some(manifest));
    assert!(holds(&store, &first).await);
    assert!(holds(&store, &second).await, "the pack's index names it");

    // Laid down in a pack rather than as one file per object, which is the whole of what a batch is
    // for: a store taking a thousand chunks writes one pack and its index rather than a thousand
    // files.
    assert!(
        !store.object_path(&first).exists(),
        "no loose object was written"
    );

    let _ = fs::remove_dir_all(&parent);
}

/// What cannot be produced is named: a chunk taken out from under a manifest is a key the store
/// cannot answer for, and that is what a sync's confirmation asks of the side that took it.
#[tokio::test]
async fn a_content_whose_chunk_is_gone_is_not_readable() {
    let (parent, store) = store("unreadable");

    let content = b"the only chunk of it";
    let chunk = store.write_object(content, Codec::Raw).await.unwrap();
    let mut manifest = Manifest::default();
    manifest.push(Chunk::new(chunk, content.len() as u64));

    // The key is the content the manifest puts back together, since that is what a key promises.
    let key = key_of(content);
    lay_manifest(&store, &key, &manifest);

    // While the chunk is there the content reads, and nothing is named.
    assert_eq!(store.unreadable(&[key]).await.unwrap(), Vec::new());

    // A manifest is not a content: without the chunk it names, the key cannot be produced, even
    // though the manifest itself still reads.
    store.remove(&chunk).await.unwrap();
    assert_eq!(store.unreadable(&[key]).await.unwrap(), vec![key]);

    let _ = fs::remove_dir_all(&parent);
}
