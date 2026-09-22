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
use super::consts::{MANIFEST_DIR, OBJECTS_DIR, PACKED_DIR};
use crate::{
    AlgorithmChoice, Chunk, Chunking, Codec, Error, FRAME_VERSION, Frame, Key, Layout, Manifest,
    StorageBackend as _, store_file,
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
    assert!(store.list_manifest_keys().await.unwrap().is_empty());

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
    store
        .write_manifest(&key, &tampered, Codec::Raw)
        .await
        .unwrap();

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

    // What a store was told to write is what it writes, and what it was told applies to
    // content of any kind.
    let told = store.with_codec(Codec::Lz4).with_cut(Chunking::Cdc {
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

    let _ = fs::remove_dir_all(&parent);
}
