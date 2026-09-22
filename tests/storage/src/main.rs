//! `storage`: what a store keeps, however the content was written in.
//!
//! A program rather than a set of tests cargo runs, because what it does is what a person does
//! with a terminal: make a store, put content into it every way it can be put, and take it back
//! out to see whether what came back is what went in.
//!
//! What it is really asking is the one thing storage promises — what goes in comes out, byte for
//! byte — asked across every codec and every cut, and then the things that follow from it: the
//! same content is the same key however it was written, an edit disturbs only the chunks it lands
//! between, compressing makes a store smaller where there is repetition, and a store that does not
//! hold together is refused rather than handed on.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use librorolala::storage::{
    AlgorithmChoice, Chunking, Codec, Error, FRAME_VERSION, Frame, Key, Layout, RorolalaStorage,
    StorageBackend as _, internals::Internals as _, store_file,
};
use rorolala_utils_sandbox::Sandbox;

#[tokio::main]
async fn main() {
    let sandbox = Sandbox::new("storage");
    let store = RorolalaStorage::create(sandbox.join("store"));
    let mut checked = Checked::default();

    round_trips(&sandbox, &store, &mut checked).await;
    cuts_show_in_the_layout(&sandbox, &store, &mut checked).await;
    an_edit_disturbs_only_its_own_chunks(&sandbox, &store, &mut checked).await;
    a_container_is_cut_at_its_members(&sandbox, &store, &mut checked).await;
    a_text_file_costs_what_an_edit_moved(&sandbox, &store, &mut checked).await;
    text_is_cut_at_the_ends_of_lines(&sandbox, &store, &mut checked).await;
    compressing_shrinks_the_store(&sandbox, &store, &mut checked).await;
    another_instance_reads_what_this_one_wrote(&sandbox, &store, &mut checked).await;
    what_is_not_there_is_not_claimed(&sandbox, &store, &mut checked).await;
    a_cut_content_whose_chunk_is_gone_is_named_but_not_held(&sandbox, &mut checked).await;
    content_that_does_not_hold_together_is_refused(&sandbox, &mut checked).await;

    sandbox.cleanup();
    checked.report();
}

/// Every codec over every cut gives back exactly what it was given.
///
/// The content written is the key's to name rather than the codec's or the cut's, so every way of
/// writing one content has to answer with one key as well.
async fn round_trips(sandbox: &Sandbox, store: &RorolalaStorage, checked: &mut Checked) {
    for (index, content) in [
        Vec::new(),
        b"short enough to leave alone".to_vec(),
        repetitive(512 * 1024),
        mixed(300 * 1024),
    ]
    .into_iter()
    .enumerate()
    {
        let file = sandbox.join(format!("round-in-{index}"));
        fs::write(&file, &content).expect("the content is written to a file");

        let mut named: Option<Key> = None;
        for choice in choices() {
            let key = match store.write_file(&file, choice).await {
                Ok(key) => key,
                Err(said) => {
                    checked.wants(&label("writes", choice), false, &said.to_string());
                    continue;
                }
            };

            match named {
                None => named = Some(key),
                Some(first) => checked.wants(
                    &label("names the content as the first write did", choice),
                    key == first,
                    &format!("{key} against {first}"),
                ),
            }

            let out = sandbox.join(format!("round-out-{index}"));
            let read = store.extract_file(&key, &out).await;
            let same = read.is_ok() && fs::read(&out).is_ok_and(|bytes| bytes == content);

            checked.wants(
                &label("gives back what it was given", choice),
                same,
                &format!("{read:?}"),
            );
        }
    }
}

/// A cut content is put back together from a manifest; one that was not cut is not.
async fn cuts_show_in_the_layout(
    sandbox: &Sandbox,
    store: &RorolalaStorage,
    checked: &mut Checked,
) {
    let content = repetitive(256 * 1024);

    let whole = sandbox.join("layout-whole");
    fs::write(&whole, &content).expect("the content is written to a file");
    let key = store
        .write_file(&whole, AlgorithmChoice::new(Codec::Raw, Chunking::Whole))
        .await
        .expect("content that is not cut is written");

    checked.wants(
        "content that was not cut is one object",
        store.object_path(&key).is_file(),
        "there is no object where the key says",
    );
    checked.wants(
        "content that was not cut has no manifest",
        !store.manifest_path(&key).exists(),
        "there is a manifest where the key says",
    );

    // The same content, cut this time: the key is the same, and so the entry the key names has to
    // change from one object to a manifest and its chunks — both are not the entry at once.
    let cut = sandbox.join("layout-cut");
    fs::write(&cut, &content).expect("the content is written to a file");
    let cut_key = store
        .write_file(
            &cut,
            AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 64 * 1024 }),
        )
        .await
        .expect("content that is cut is written");

    checked.wants(
        "cutting a content does not change its key",
        cut_key == key,
        &format!("{cut_key} against {key}"),
    );
    checked.wants(
        "content that was cut has a manifest",
        store.manifest_path(&key).is_file(),
        "there is no manifest where the key says",
    );

    // Written both ways, the key still names one content: the object a whole write left and the
    // chunks a cut write left come to the same bytes, so either is the right answer to hand back.
    let out = sandbox.join("layout-out");
    let read = store.extract_file(&key, &out).await;
    checked.wants(
        "the same content written both ways reads back the same",
        read.is_ok() && fs::read(&out).is_ok_and(|bytes| bytes == content),
        &format!("{read:?}"),
    );

    let manifest = store
        .read_manifest(&key)
        .await
        .expect("the manifest is read")
        .expect("the entry is a manifest");

    checked.wants(
        "the manifest names more than one chunk",
        manifest.len() >= 2,
        &format!("{} chunk(s)", manifest.len()),
    );
    checked.wants(
        "every chunk the manifest names is an object",
        manifest
            .chunks()
            .iter()
            .all(|chunk| store.object_path(&chunk.key()).is_file()),
        "a chunk the manifest names is not there",
    );
    checked.wants(
        "the chunks add up to the content",
        manifest.content_len() == content.len() as u64,
        &format!("{} against {}", manifest.content_len(), content.len()),
    );
}

/// An edit adds only the chunks it lands in, when the cut follows the content rather than position.
async fn an_edit_disturbs_only_its_own_chunks(
    sandbox: &Sandbox,
    store: &RorolalaStorage,
    checked: &mut Checked,
) {
    let before = mixed(2 << 20);
    let mut after = before.clone();
    after.insert(before.len() / 2, 0x7F);

    let cdc = AlgorithmChoice::new(
        Codec::Raw,
        Chunking::Cdc {
            min: 16 * 1024,
            average: 64 * 1024,
            max: 256 * 1024,
        },
    );
    let fixed = AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 64 * 1024 });

    // Cut on content, the chunks either side of the edit are found in the same places again, so
    // what is new is what the edit actually landed in.
    let _ = objects_gained(sandbox, store, "edit-cdc-before", &before, cdc).await;
    let cdc_gained = objects_gained(sandbox, store, "edit-cdc-after", &after, cdc).await;

    checked.wants(
        "an edit adds only the chunks it lands in",
        cdc_gained <= 4,
        &format!("{cdc_gained} objects were added"),
    );

    // Cut by position, everything from the edit on lands somewhere else, so the whole tail is new.
    let _ = objects_gained(sandbox, store, "edit-fixed-before", &before, fixed).await;
    let fixed_gained = objects_gained(sandbox, store, "edit-fixed-after", &after, fixed).await;

    checked.wants(
        "a positional cut does not survive an edit",
        fixed_gained > cdc_gained,
        &format!("{fixed_gained} against {cdc_gained}"),
    );
}

/// A packed container is taken apart where it is packed, so an edit to one member is cheap — and
/// what comes back out is still the container that went in, byte for byte.
async fn a_container_is_cut_at_its_members(
    sandbox: &Sandbox,
    store: &RorolalaStorage,
    checked: &mut Checked,
) {
    let member = [1_u8; 64 * 1024];
    let before = sandbox.join("deck-before.pptx");
    let after = sandbox.join("deck-after.pptx");

    fs::write(
        &before,
        zipped(&[("slide1", &member), ("slide2", b"a slide")]),
    )
    .expect("the deck is written to a file");
    fs::write(
        &after,
        zipped(&[("slide1", &member), ("slide2", b"A SLIDE")]),
    )
    .expect("the deck is written to a file");

    // The cut is the store's own choice rather than something a caller has to know to ask for, and
    // a container that is already packed is not packed again.
    let choice = store.choose_algorithm(&before);
    checked.wants(
        "a container is cut at its members by the store's own choice",
        choice.chunking() == Chunking::Zip,
        &format!("{:?}", choice.chunking()),
    );
    checked.wants(
        "a container that is packed already is not compressed again",
        choice.codec() == Codec::Raw,
        &format!("{:?}", choice.codec()),
    );

    let _ = store_file(store, &before)
        .await
        .expect("the deck before the edit is stored");
    let stored = objects(sandbox);

    let key = store_file(store, &after)
        .await
        .expect("the deck after the edit is stored");
    let gained = objects(sandbox) - stored;

    // One member changed, so one member is new: the member the edit did not touch is the bytes it
    // already was, and the store already has them. The manifest is new as well, which is why this
    // is one or two rather than none.
    checked.wants(
        "an edit to one member is one new member",
        gained <= 3,
        &format!("{gained} objects were added"),
    );

    let out = sandbox.join("deck-out.pptx");
    let read = store.extract_file(&key, &out).await;

    checked.wants(
        "a container comes back out as the one that went in",
        read.is_ok() && fs::read(&out).is_ok_and(|bytes| bytes == fs::read(&after).unwrap()),
        &format!("{read:?}"),
    );
}

/// Text is kept the way an edit moves it: a few lines, not the file.
async fn a_text_file_costs_what_an_edit_moved(
    sandbox: &Sandbox,
    store: &RorolalaStorage,
    checked: &mut Checked,
) {
    let before = sandbox.join("text-before.rs");
    let after = sandbox.join("text-after.rs");
    fs::write(&before, text_lines(0, 20_000)).expect("the text is written to a file");

    // The same text with one line added in the middle of it.
    let mut edited = text_lines(0, 10_000);
    edited.extend_from_slice(b"a line that was added\n");
    edited.extend_from_slice(&text_lines(10_000, 20_000));
    fs::write(&after, &edited).expect("the text is written to a file");

    // Text is compressed, and cut at the ends of its lines once there is enough of it.
    let choice = store.choose_algorithm(&before);
    checked.wants(
        "text is compressed",
        choice.codec() == Codec::Zstd,
        &format!("{:?}", choice.codec()),
    );
    checked.wants(
        "text worth cutting is cut",
        matches!(choice.chunking(), Chunking::Text { .. }),
        &format!("{:?}", choice.chunking()),
    );

    let _ = store_file(store, &before)
        .await
        .expect("the text before the edit is stored");
    let stored = objects(sandbox);

    let key = store_file(store, &after)
        .await
        .expect("the text after the edit is stored");
    let gained = objects(sandbox) - stored;

    checked.wants(
        "an added line costs a chunk or two, not the file",
        gained <= 4,
        &format!("{gained} objects were added"),
    );

    let out = sandbox.join("text-out.rs");
    let read = store.extract_file(&key, &out).await;

    checked.wants(
        "text comes back out as the text that went in",
        read.is_ok() && fs::read(&out).is_ok_and(|bytes| bytes == edited),
        &format!("{read:?}"),
    );
}

/// Text is cut where its lines end, so no chunk ever stops inside a line.
///
/// Cutting text at line ends is what lets an edit be cheap: a chunk can only be shared with another
/// version of a file when it is made of lines both versions have, and a chunk that began or ended in
/// the middle of a line would be one no other version could hold. Nothing in the layout records the
/// cut, so this is asked of the store — every chunk's own bytes are read back and looked at.
async fn text_is_cut_at_the_ends_of_lines(
    sandbox: &Sandbox,
    store: &RorolalaStorage,
    checked: &mut Checked,
) {
    let text = sandbox.join("ends.rs");
    let content = text_lines(0, 6_000);
    fs::write(&text, &content).expect("the text is written to a file");

    let choice = store.choose_algorithm(&text);
    checked.wants(
        "text worth cutting is cut",
        matches!(choice.chunking(), Chunking::Text { .. }),
        &format!("{:?}", choice.chunking()),
    );

    let key = store_file(store, &text).await.expect("the text is stored");
    let manifest = store
        .read_manifest(&key)
        .await
        .expect("the manifest is read")
        .expect("text that was cut has a manifest");

    let mut ends_well = true;
    let mut pieces = Vec::new();

    for (at, chunk) in manifest.chunks().iter().enumerate() {
        let bytes = store
            .read_object(&chunk.key())
            .await
            .expect("every chunk of stored text is read back");

        // Every chunk but the last has to end at a line end; the last is free to be the file's own
        // tail, which need not end a line.
        if at + 1 < manifest.len() && !bytes.ends_with(b"\n") {
            ends_well = false;
        }

        pieces.extend_from_slice(&bytes);
    }

    checked.wants(
        "no chunk stops inside a line",
        ends_well,
        "a chunk ended in the middle of a line",
    );
    checked.wants(
        "the chunks are the text and nothing beside it",
        pieces == content,
        "what the chunks add up to is not the text that went in",
    );

    // Text too small to be worth the bookkeeping is kept whole: cutting it would cost a manifest
    // and share nothing.
    let small = sandbox.join("small.txt");
    fs::write(&small, b"a line\nanother line\n").expect("the text is written to a file");

    checked.wants(
        "text too small to cut is kept whole",
        store.choose_algorithm(&small).chunking() == Chunking::Whole,
        "small text was cut anyway",
    );

    let key = store_file(store, &small).await.expect("the text is stored");
    checked.wants(
        "text kept whole has no manifest",
        store
            .read_manifest(&key)
            .await
            .expect("the manifest is read")
            .is_none(),
        "text that was not cut has a manifest",
    );

    // How a line ends, and whether the last one ends at all, is the content's business: either way
    // the text comes back out as it went in.
    let mut unterminated = text_lines(0, 5_000);
    let _ = unterminated.pop();

    let mut windows = String::new();
    for line in 0..5_000 {
        let _ = writeln!(windows, "line {line}\r");
    }

    for (name, written) in [
        ("unterminated.rs", unterminated),
        ("windows.rs", windows.into_bytes()),
    ] {
        let file = sandbox.join(name);
        fs::write(&file, &written).expect("the text is written to a file");

        let key = store_file(store, &file).await.expect("the text is stored");
        let out = sandbox.join(format!("out-{name}"));
        let read = store.extract_file(&key, &out).await;

        checked.wants(
            "text with its own line endings comes back as it went in",
            read.is_ok() && fs::read(&out).is_ok_and(|bytes| bytes == written),
            &format!("{read:?}"),
        );
    }
}

/// Compressing makes a store smaller where there is repetition to find.
async fn compressing_shrinks_the_store(
    sandbox: &Sandbox,
    store: &RorolalaStorage,
    checked: &mut Checked,
) {
    let _ = store;
    let content = repetitive(1 << 20);
    let mut sizes = Vec::new();

    // A store of its own for each codec, so what is measured is what that one write cost rather
    // than what the last one left behind.
    for (label, codec) in [
        ("raw", Codec::Raw),
        ("lz4", Codec::Lz4),
        ("zstd", Codec::Zstd),
    ] {
        let root = sandbox.join(format!("compress-{label}"));
        let store = RorolalaStorage::create(&root);
        let file = sandbox.join(format!("compress-in-{label}"));
        fs::write(&file, &content).expect("the content is written to a file");

        store
            .write_file(&file, AlgorithmChoice::new(codec, Chunking::Whole))
            .await
            .expect("the content is written");

        sizes.push((label, bytes_in(&root.join("obj"))));
    }

    let (raw, lz4, zstd) = (sizes[0].1, sizes[1].1, sizes[2].1);

    checked.wants(
        "lz4 makes repetition smaller",
        lz4 < raw,
        &format!("{lz4} against {raw}"),
    );
    checked.wants(
        "zstd makes repetition smaller still",
        zstd <= lz4,
        &format!("{zstd} against {lz4}"),
    );
}

/// A store is a place rather than a process: another instance reads what this one wrote.
async fn another_instance_reads_what_this_one_wrote(
    sandbox: &Sandbox,
    store: &RorolalaStorage,
    checked: &mut Checked,
) {
    let content = mixed(400 * 1024);
    let file = sandbox.join("reopen-in");
    fs::write(&file, &content).expect("the content is written to a file");

    let key = store
        .write_file(
            &file,
            AlgorithmChoice::new(Codec::Zstd, Chunking::Fixed { size: 32 * 1024 }),
        )
        .await
        .expect("the content is written");

    let reopened = RorolalaStorage::at(sandbox.join("store"))
        .expect("a store is found by the configuration it carries");

    let out = sandbox.join("reopen-out");
    let read = reopened.extract_file(&key, &out).await;

    checked.wants(
        "another instance reads what this one wrote",
        read.is_ok() && fs::read(&out).is_ok_and(|bytes| bytes == content),
        &format!("{read:?}"),
    );
    checked.wants(
        "another instance holds the same keys",
        reopened
            .list_all_keys()
            .await
            .is_ok_and(|keys| keys.contains(&key)),
        "the key is not among the ones it lists",
    );
}

/// What is not there is not claimed to be, and what is listed can be taken back out.
async fn what_is_not_there_is_not_claimed(
    sandbox: &Sandbox,
    store: &RorolalaStorage,
    checked: &mut Checked,
) {
    let absent = Key::new([0x5A; 32]);

    checked.wants(
        "a key nothing is under is not held",
        !holds(store, &absent).await,
        "it was claimed to be held",
    );
    checked.wants(
        "a key nothing is under is not handed back",
        matches!(
            store
                .extract_file(&absent, &sandbox.join("absent-out"))
                .await,
            Err(Error::NotFound(_))
        ),
        "something was handed back for it",
    );

    // What a store can take back out is what it lists as existing: the two questions are one, asked
    // of every key rather than of one.
    let named = store.list_all_keys().await.unwrap_or_default();
    let exist = store.list_exist_keys().await.unwrap_or_default();

    checked.wants(
        "everything named is one it can take back out",
        exist == named,
        "a key it names is one it does not hold",
    );
}

/// A cut content whose chunk has gone is named by a store but not held by it.
///
/// This is the one place the two lists differ: the manifest is still there and names a chunk that is
/// not, so the content is named and cannot be put back together.
async fn a_cut_content_whose_chunk_is_gone_is_named_but_not_held(
    sandbox: &Sandbox,
    checked: &mut Checked,
) {
    let store = RorolalaStorage::create(sandbox.join("missing-chunk"));
    let content = repetitive(64 * 1024);
    let file = sandbox.join("missing-chunk-content");
    fs::write(&file, &content).expect("the content is written to a file");

    let key = store
        .write_file(
            &file,
            AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 4096 }),
        )
        .await
        .expect("the cut content is written");

    let manifest = store
        .read_manifest(&key)
        .await
        .expect("the manifest is read")
        .expect("cut content has a manifest");
    let chunk = manifest.chunks()[0].key();

    store.remove(&chunk).await.expect("the chunk is dropped");

    let named = store.list_all_keys().await.unwrap_or_default();
    let held = store.list_exist_keys().await.unwrap_or_default();

    checked.wants(
        "a content whose chunk is gone is still named",
        named.contains(&key),
        "the content is not named at all",
    );
    checked.wants(
        "a content whose chunk is gone is not held",
        !held.contains(&key),
        "the content is held with a chunk missing",
    );
    checked.wants(
        "everything held is named",
        held.iter().all(|key| named.contains(key)),
        "a key is held but not named",
    );
}

/// What is stored under a key but does not hold together is refused, not handed on.
async fn content_that_does_not_hold_together_is_refused(sandbox: &Sandbox, checked: &mut Checked) {
    // A store of its own, so that the content is not also under the key as one object: a whole copy
    // beside a broken manifest is content the key can still answer with, and this asks what happens
    // when there is no such copy.
    let store = RorolalaStorage::create(sandbox.join("corrupt"));
    let content = mixed(300 * 1024);
    let file = sandbox.join("corrupt-in");
    fs::write(&file, &content).expect("the content is written to a file");

    let key = store
        .write_file(
            &file,
            AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 32 * 1024 }),
        )
        .await
        .expect("the content is written");

    let manifest = store
        .read_manifest(&key)
        .await
        .expect("the manifest is read")
        .expect("the entry is a manifest");
    let chunk = store.object_path(&manifest.chunks()[0].key());

    // Something else entirely where a chunk was: framing no build can read.
    fs::write(&chunk, b"not a chunk at all").expect("the chunk is written over");
    let out = sandbox.join("corrupt-out");

    checked.wants(
        "a chunk that will not read is refused",
        matches!(
            store.extract_file(&key, &out).await,
            Err(Error::Malformed) | Err(Error::Corrupt(_))
        ),
        "something was handed back for it",
    );

    // And something framed like a chunk but holding another content: it reads, and it is still not
    // the content the key asked for.
    let framed = Frame {
        version: FRAME_VERSION,
        codec: Codec::Raw,
        layout: Layout::Single,
        plain_len: 5,
    };
    let mut bytes = framed.encode();
    bytes.extend_from_slice(b"other");
    fs::write(&chunk, bytes).expect("the chunk is written over");

    checked.wants(
        "a chunk holding another content is refused",
        matches!(store.extract_file(&key, &out).await, Err(Error::Corrupt(_))),
        "something was handed back for it",
    );
}

/// Every codec over every cut a store can be asked for.
fn choices() -> Vec<AlgorithmChoice> {
    let mut choices = Vec::new();

    for codec in [Codec::Raw, Codec::Lz4, Codec::Zstd] {
        for chunking in [
            Chunking::Whole,
            Chunking::Fixed { size: 64 * 1024 },
            Chunking::Cdc {
                min: 16 * 1024,
                average: 64 * 1024,
                max: 256 * 1024,
            },
        ] {
            choices.push(AlgorithmChoice::new(codec, chunking));
        }
    }

    choices
}

/// How a choice reads in a question asked of it.
fn label(asked: &str, choice: AlgorithmChoice) -> String {
    format!("{:?} over {:?} {asked}", choice.codec(), choice.chunking())
}

/// Text of whole numbered lines, from `from` up to `to`, each ended the way the file says.
fn text_lines(from: usize, to: usize) -> Vec<u8> {
    let mut text = String::new();
    for line in from..to {
        let _ = writeln!(text, "line {line}");
    }

    text.into_bytes()
}

/// Content with repetition in it, so a codec has something to find.
fn repetitive(len: usize) -> Vec<u8> {
    b"the same words over and over, "
        .iter()
        .cycle()
        .take(len)
        .copied()
        .collect()
}

/// Content of both kinds at once: half repetition, half noise, so neither compressing nor cutting
/// is handed something it cannot do anything with.
fn mixed(len: usize) -> Vec<u8> {
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;

    (0..len)
        .map(|index| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);

            if index % 2 == 0 {
                b"aA"[index % 2]
            } else {
                state.to_le_bytes()[4]
            }
        })
        .collect()
}

/// Whether `store` can produce `key`, asked of it in a batch of one.
async fn holds(store: &RorolalaStorage, key: &Key) -> bool {
    store
        .contains_keys(&[*key])
        .await
        .unwrap_or_default()
        .held(0)
}

/// Writes `content` the way `choice` says, and answers how many objects the store gained.
async fn objects_gained(
    sandbox: &Sandbox,
    store: &RorolalaStorage,
    name: &str,
    content: &[u8],
    choice: AlgorithmChoice,
) -> usize {
    let before = objects(sandbox);
    let file = sandbox.join(name);
    fs::write(&file, content).expect("the content is written to a file");

    store
        .write_file(&file, choice)
        .await
        .expect("the content is written");

    objects(sandbox) - before
}

/// How many objects the store of the sandbox holds under `obj/`.
fn objects(sandbox: &Sandbox) -> usize {
    files(&sandbox.join("store/obj")).len()
}

/// A packed container of `members`, stored rather than deflated.
///
/// This is the shape a chunker reads — members one after another, with a directory saying where
/// each one is — rather than the shape a packer would choose, which is what is being tested.
fn zipped(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut local = Vec::new();
    let mut directory = Vec::new();

    for (name, data) in members {
        let name = name.as_bytes();
        let length = u16::try_from(name.len()).unwrap();
        let size = u32::try_from(data.len()).unwrap();
        let offset = u32::try_from(local.len()).unwrap();

        local.extend_from_slice(b"PK\x03\x04"); // signature
        local.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // needed, flags, method, time, date
        local.extend_from_slice(&[0; 4]); // crc
        local.extend_from_slice(&size.to_le_bytes()); // compressed
        local.extend_from_slice(&size.to_le_bytes()); // uncompressed
        local.extend_from_slice(&length.to_le_bytes()); // name
        local.extend_from_slice(&[0, 0]); // extra
        local.extend_from_slice(name);
        local.extend_from_slice(data);

        directory.extend_from_slice(b"PK\x01\x02"); // signature
        directory.extend_from_slice(&[20, 0, 20, 0]); // made by, needed
        directory.extend_from_slice(&[0; 8]); // flags, method, time, date
        directory.extend_from_slice(&[0; 4]); // crc
        directory.extend_from_slice(&size.to_le_bytes()); // compressed
        directory.extend_from_slice(&size.to_le_bytes()); // uncompressed
        directory.extend_from_slice(&length.to_le_bytes()); // name
        directory.extend_from_slice(&[0; 8]); // extra, comment, disk, internal
        directory.extend_from_slice(&[0; 4]); // external
        directory.extend_from_slice(&offset.to_le_bytes()); // where its header is
        directory.extend_from_slice(name);
    }

    let mut end = Vec::new();
    end.extend_from_slice(b"PK\x05\x06"); // signature
    end.extend_from_slice(&[0; 4]); // disk, directory's disk
    end.extend_from_slice(&u16::try_from(members.len()).unwrap().to_le_bytes()); // here
    end.extend_from_slice(&u16::try_from(members.len()).unwrap().to_le_bytes()); // in all
    end.extend_from_slice(&u32::try_from(directory.len()).unwrap().to_le_bytes());
    end.extend_from_slice(&u32::try_from(local.len()).unwrap().to_le_bytes());
    end.extend_from_slice(&[0, 0]); // comment

    let mut content = local;
    content.extend_from_slice(&directory);
    content.extend_from_slice(&end);

    content
}

/// The files under `directory`, however deep.
fn files(directory: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(directory) else {
        return found;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            found.extend(files(&path));
        } else {
            found.push(path);
        }
    }

    found
}

/// How many bytes the files under `directory` add up to.
fn bytes_in(directory: &Path) -> u64 {
    files(directory)
        .iter()
        .filter_map(|path| fs::metadata(path).ok())
        .map(|data| data.len())
        .sum()
}

/// What was asked of the store, and how many of the questions were answered as they should be.
#[derive(Default)]
struct Checked {
    /// How many questions were asked.
    asked: usize,
    /// Which of them were answered with something other than what was wanted.
    wrong: Vec<String>,
}

impl Checked {
    /// Counts a question, and records it as wrong when it was not answered as it should be.
    fn wants(&mut self, question: &str, answered: bool, said: &str) {
        self.asked += 1;

        if !answered {
            self.wrong.push(format!("{question} — {said}"));
        }
    }

    /// Says what was asked and what was wrong with the answers, and ends the program on them.
    fn report(self) {
        for said in &self.wrong {
            eprintln!("FAIL: {said}");
        }

        if self.wrong.is_empty() {
            println!(
                "{} questions, every one answered as it should be",
                self.asked
            );
            return;
        }

        eprintln!(
            "{} of {} answered as they should not",
            self.wrong.len(),
            self.asked
        );
        std::process::exit(1);
    }
}
