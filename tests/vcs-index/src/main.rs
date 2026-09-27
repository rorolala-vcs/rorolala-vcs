//! `vcs-index`: what the version control index answers as its objects are packed and moved.
//!
//! The index is a store of its own, addressed by the same kind of key a store is: a Variant or a
//! Version is written under its own hash and read back by it, whether it sits loose or in a pack.
//! That is the whole promise, and this is where it is asked of an index that has actually been
//! packed — and of what a `rola pack` leaves behind.
//!
//! A program rather than a set of tests cargo runs, for the same reason the rest of them are: what
//! it does is what a person does with a terminal, and what it checks is what a program would say
//! back.

use std::fs;
use std::path::{Path, PathBuf};

use librorolala::storage::{StorageBackend as _, transfer};
use librorolala::vcs::{
    Creator, Message, ROOT_VERSION, VCSIndex, VCSIndexObject, VCSWrite as _, Variant, Version,
};
use rorolala_utils_sandbox::{Guard, Sandbox};

#[tokio::main]
async fn main() {
    let sandbox = Guard::new("vcs-index");
    let mut checked = Checked::default();

    a_variant_round_trips_through_an_index(&sandbox, &mut checked).await;
    a_version_round_trips_through_an_index(&sandbox, &mut checked).await;
    repacking_lays_loose_objects_into_one_pack(&sandbox, &mut checked).await;
    a_packed_object_reads_through_a_fresh_handle(&sandbox, &mut checked).await;
    removing_a_packed_object_takes_it_out_of_the_pack(&sandbox, &mut checked).await;
    an_object_moves_between_two_indexes_over_a_transfer(&sandbox, &mut checked).await;
    a_version_number_is_worked_out_by_tracing_the_chain(&sandbox, &mut checked).await;
    a_creator_and_a_message_round_trip_and_survive_a_pack(&sandbox, &mut checked).await;

    checked.report();
}

/// An index of its own for one question, under `name` in the sandbox.
fn index(sandbox: &Sandbox, name: &str) -> VCSIndex {
    VCSIndex::create(sandbox.join(name))
}

/// A progressive Variant of this question's own, so two of them never collide.
fn variant(seed: u8) -> Variant {
    Variant::new_bare_variant(
        [seed; 32],
        [seed.wrapping_add(1); 32],
        None,
        [0; 32],
        [0; 32],
        0,
    )
}

/// A Variant written under its own hash and read back is the very Variant that was written.
async fn a_variant_round_trips_through_an_index(sandbox: &Sandbox, checked: &mut Checked) {
    let index = index(sandbox, "variant");
    let variant = variant(1);

    let key = index
        .write(variant.clone())
        .await
        .expect("the variant is written");
    let read = index.read(key).await.expect("the variant is read");

    checked.wants(
        "a variant read back is the one written",
        read == VCSIndexObject::Variant(variant.clone()),
        "the variant changed on the way through",
    );
    checked.wants(
        "the key an object is stored under is its own hash",
        key == variant.hash(),
        "the key is not the object's hash",
    );
}

/// A Version written under its own hash and read back is the very Version that was written.
async fn a_version_round_trips_through_an_index(sandbox: &Sandbox, checked: &mut Checked) {
    let index = index(sandbox, "version");
    let version = Version::new_bare_version([7; 32], 0);

    let key = index
        .write(version.clone())
        .await
        .expect("the version is written");
    let read = index.read(key).await.expect("the version is read");

    checked.wants(
        "a version read back is the one written",
        read == VCSIndexObject::Version(version),
        "the version changed on the way through",
    );
}

/// Packing lays the loose objects into one pack, and every read that worked loose still works.
async fn repacking_lays_loose_objects_into_one_pack(sandbox: &Sandbox, checked: &mut Checked) {
    let index = index(sandbox, "repack");
    let one = variant(2);
    let other = Version::new_bare_version([9; 32], 0);

    index
        .write(one.clone())
        .await
        .expect("the variant is written");
    index
        .write(other.clone())
        .await
        .expect("the version is written");

    checked.wants(
        "the objects are loose to begin with",
        objects(sandbox, "repack") == 2,
        "the objects were not both loose",
    );

    let made = index.repack().await.expect("the index is packed");
    checked.wants("packing makes a pack", made, &format!("{made}"));
    checked.wants(
        "nothing is left loose once it is packed",
        objects(sandbox, "repack") == 0,
        "an object was still loose after it was packed",
    );
    checked.wants(
        "the pack and the index beside it are there",
        sandbox.join("repack/packed/packed_0.pack").is_file()
            && sandbox.join("repack/packed/packed_0.idx").is_file(),
        "the pack or its index is not there",
    );

    checked.wants(
        "packing an index already laid out changes nothing",
        !index.repack().await.expect("the index is packed again"),
        "the second packing changed something",
    );

    checked.wants(
        "a packed variant reads back as it was written",
        index.read(one.hash()).await.expect("the variant is read") == VCSIndexObject::Variant(one),
        "the packed variant did not read back",
    );
    checked.wants(
        "a packed version reads back as it was written",
        index.read(other.hash()).await.expect("the version is read")
            == VCSIndexObject::Version(other),
        "the packed version did not read back",
    );
}

/// A packed object is reached through a handle made afresh at the same directory.
async fn a_packed_object_reads_through_a_fresh_handle(sandbox: &Sandbox, checked: &mut Checked) {
    let index = index(sandbox, "reopen");
    let variant = variant(3);
    let key = index
        .write(variant.clone())
        .await
        .expect("the variant is written");
    index.repack().await.expect("the index is packed");

    let reopened = VCSIndex::at(sandbox.join("reopen")).expect("the index is there");

    checked.wants(
        "a packed object reads through a fresh handle",
        reopened.read(key).await.expect("the variant is read") == VCSIndexObject::Variant(variant),
        "the packed object did not read through a fresh handle",
    );
}

/// Taking a packed object away leaves the ones beside it in the pack readable.
async fn removing_a_packed_object_takes_it_out_of_the_pack(
    sandbox: &Sandbox,
    checked: &mut Checked,
) {
    let index = index(sandbox, "remove");
    let kept = variant(4);
    let removed = Version::new_bare_version([5; 32], 0);
    let kept_key = index
        .write(kept.clone())
        .await
        .expect("the variant is written");
    let removed_key = index.write(removed).await.expect("the version is written");
    index.repack().await.expect("the index is packed");

    index
        .remove(&removed_key)
        .await
        .expect("the version is dropped");

    let presence = index
        .contains_keys(&[kept_key, removed_key])
        .await
        .expect("the index is asked");
    checked.wants(
        "the object that was not dropped is still held",
        presence.held(0),
        "the untouched object was dropped too",
    );
    checked.wants(
        "the object that was dropped is gone",
        !presence.held(1),
        "the dropped object is still held",
    );
    checked.wants(
        "the object beside it still reads back",
        index.read(kept_key).await.expect("the variant is read") == VCSIndexObject::Variant(kept),
        "the untouched object did not read back",
    );
}

/// An object written at one index arrives at another over a transfer.
async fn an_object_moves_between_two_indexes_over_a_transfer(
    sandbox: &Sandbox,
    checked: &mut Checked,
) {
    let left = index(sandbox, "transfer-left");
    let right = index(sandbox, "transfer-right");
    let variant = variant(6);
    let key = left
        .write(variant.clone())
        .await
        .expect("the variant is written");

    // The two ends are driven over a pair of buffers in one process, which is all a transfer needs:
    // the stream is the caller's, not the index's.
    let (one, other) = tokio::io::duplex(4096);
    let keys = [key];
    let (left_side, right_side) = tokio::join!(
        transfer::initiate(&left, one, &keys),
        transfer::respond(&right, other),
    );
    left_side.expect("the sender finishes");
    right_side.expect("the receiver finishes");

    checked.wants(
        "the object that moved is read at the other end",
        right.read(key).await.expect("the variant is read") == VCSIndexObject::Variant(variant),
        "the object did not arrive whole",
    );
}

/// A version's number is not stored: it is worked out by tracing the chain back to the root.
async fn a_version_number_is_worked_out_by_tracing_the_chain(
    sandbox: &Sandbox,
    checked: &mut Checked,
) {
    let index = index(sandbox, "chain");

    // root -> variant -> version 0 -> variant -> version 1, with a merge off the second variant
    // that stands where the variant it merges stands.
    let root = Version::root();
    index
        .write(root.clone())
        .await
        .expect("the root is written");

    let first_variant = root.new_variant([0x11; 32], [0xaa; 32], [0xbb; 32]);
    index
        .write(first_variant.clone())
        .await
        .expect("the variant is written");
    let first = first_variant.new_version();
    index
        .write(first.clone())
        .await
        .expect("the version is written");

    let second_variant = first.new_variant([0x22; 32], [0xcc; 32], [0xdd; 32]);
    index
        .write(second_variant.clone())
        .await
        .expect("the variant is written");
    let second = second_variant.new_version();
    index
        .write(second.clone())
        .await
        .expect("the version is written");

    let merge = second_variant.new_merge_variant([0x33; 32], &first_variant);
    index
        .write(merge.clone())
        .await
        .expect("the merge is written");
    let merged = merge.new_version();
    index
        .write(merged.clone())
        .await
        .expect("the version is written");

    checked.wants(
        "the root is numbered one before the first version",
        index
            .version_num(&root)
            .await
            .expect("a number is worked out")
            == ROOT_VERSION,
        "the root was not -1",
    );
    checked.wants(
        "the first version is numbered from nothing",
        index
            .version_num(&first)
            .await
            .expect("a number is worked out")
            == 0,
        "the first version was not 0",
    );
    checked.wants(
        "the version made of the next variant is numbered one on",
        index
            .version_num(&second)
            .await
            .expect("a number is worked out")
            == 1,
        "the second version was not 1",
    );
    checked.wants(
        "a merge stands where the variant it merges stands",
        index
            .version_num(&merged)
            .await
            .expect("a number is worked out")
            == 1,
        "the merged version did not stand still",
    );
    checked.wants(
        "a variant is numbered with its base version",
        index
            .variant_num(&second_variant)
            .await
            .expect("a number is worked out")
            == 0,
        "the variant was not numbered with its base version",
    );
}

/// A Creator and a Message are text objects: written, read back, packed, read again.
async fn a_creator_and_a_message_round_trip_and_survive_a_pack(
    sandbox: &Sandbox,
    checked: &mut Checked,
) {
    let index = index(sandbox, "text");
    let creator = Creator::try_from("Wei Cao").expect("the name fits");
    let message = Message::try_from("made the first cut").expect("the message fits");

    let creator_key = index
        .write(creator.clone())
        .await
        .expect("the creator is written");
    let message_key = index
        .write(message.clone())
        .await
        .expect("the message is written");

    checked.wants(
        "a creator read back is the one written",
        index.read(creator_key).await.expect("the creator is read")
            == VCSIndexObject::Creator(creator.clone()),
        "the creator changed on the way through",
    );
    checked.wants(
        "a message read back is the one written",
        index.read(message_key).await.expect("the message is read")
            == VCSIndexObject::Message(message.clone()),
        "the message changed on the way through",
    );

    // The pack takes everything under `obj/`, whatever kind it is, so both go into it and read
    // back the same.
    index.repack().await.expect("the index is packed");

    checked.wants(
        "a packed creator reads back as it was written",
        index.read(creator_key).await.expect("the creator is read")
            == VCSIndexObject::Creator(creator),
        "the packed creator did not read back",
    );
    checked.wants(
        "a packed message reads back as it was written",
        index.read(message_key).await.expect("the message is read")
            == VCSIndexObject::Message(message),
        "the packed message did not read back",
    );
}

/// How many loose objects the index of `name` holds under `obj/`.
fn objects(sandbox: &Sandbox, name: &str) -> usize {
    files(&sandbox.join(name).join("obj")).len()
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

/// What was asked of the index, and how many of the questions were answered as they should be.
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
