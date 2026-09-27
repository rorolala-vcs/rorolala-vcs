//! `inverse-index`: what the reverse dependency index answers, and what it falls back to.
//!
//! The index is content addressed, so it answers "what is stored under this hash" and nothing the
//! other way. The inverse index is the other way — which objects depend on a key — kept as records
//! beside the objects, and built by reading every object once. It is derived data, so what matters
//! is that an answer is the same whether it came from the records or from the objects, that it is
//! right the moment an object is written the records do not describe, and that what is answered does
//! not change when the objects are packed.
//!
//! A program rather than a set of tests cargo runs, for the same reason the rest of them are.

use librorolala::inverse_index::InverseIndex;
use librorolala::storage::Key;
use librorolala::vcs::{ROOT_VERSION, VCSIndex, VCSWrite as _, Variant, Version};
use rorolala_utils_sandbox::{Guard, Sandbox};

#[tokio::main]
async fn main() {
    let sandbox = Guard::new("inverse-index");
    let mut checked = Checked::default();

    a_rebuild_records_what_points_at_what(&sandbox, &mut checked).await;
    a_write_the_records_do_not_describe_falls_back(&sandbox, &mut checked).await;
    a_rebuild_takes_in_what_was_written_after(&sandbox, &mut checked).await;
    a_pack_does_not_change_what_is_answered(&sandbox, &mut checked).await;
    nothing_is_answered_as_stored_when_nothing_is_there(&sandbox, &mut checked).await;

    checked.report();
}

/// An index of its own for one question, under `name` in the sandbox.
fn index(sandbox: &Sandbox, name: &str) -> VCSIndex {
    VCSIndex::create(sandbox.join(name))
}

/// The inverse index over `index`.
fn inverse(index: &VCSIndex) -> InverseIndex {
    InverseIndex::at(index.clone())
}

/// A chain of root → variant → version written into `index`.
///
/// The storage address, the creator and the message are the hashes the variant names with the bytes
/// `0x11`, `0xaa` and `0xbb`, so a question can be asked of each without reading anything back.
struct Chain {
    /// The hash of the root version.
    root: Key,
    /// The variant based on the root.
    variant: Variant,
    /// The version that fixes the variant.
    version: Version,
    /// The content the variant was made of.
    storage: Key,
    /// The creator the variant names.
    creator: Key,
    /// The message the variant names.
    message: Key,
}

async fn chain(index: &VCSIndex) -> Chain {
    let root = Version::root();
    index
        .write(root.clone())
        .await
        .expect("the root is written");

    let variant = root.new_variant([0x11; 32], [0xaa; 32], [0xbb; 32]);
    index
        .write(variant.clone())
        .await
        .expect("the variant is written");

    let version = variant.new_version();
    index
        .write(version.clone())
        .await
        .expect("the version is written");

    Chain {
        root: root.hash(),
        variant,
        version,
        storage: Key::new([0x11; 32]),
        creator: Key::new([0xaa; 32]),
        message: Key::new([0xbb; 32]),
    }
}

/// The records built from the objects answer every dependency the objects hold.
async fn a_rebuild_records_what_points_at_what(sandbox: &Sandbox, checked: &mut Checked) {
    let index = index(sandbox, "records");
    let chain = chain(&index).await;
    let inverse = inverse(&index);

    let report = inverse.rebuild().await.expect("the inverse index is built");
    checked.wants(
        "a rebuild reads every object the index holds",
        report.objects == 3,
        &format!("{} objects were read", report.objects),
    );

    checked.wants(
        "a variant depends on the content it was made of",
        inverse
            .store_dependents(chain.storage)
            .await
            .expect("the records answer")
            == vec![chain.variant.hash()],
        "the variant was not found under its storage address",
    );
    checked.wants(
        "a version depends on the variant it fixes",
        inverse
            .variant_dependents(chain.variant.hash())
            .await
            .expect("the records answer")
            == vec![chain.version.hash()],
        "the version was not found under its variant",
    );
    checked.wants(
        "a variant depends on the version it is based on",
        inverse
            .version_dependents(chain.root)
            .await
            .expect("the records answer")
            == vec![chain.variant.hash()],
        "the variant was not found under its base version",
    );
    checked.wants(
        "a variant depends on the creator that made it",
        inverse
            .creator_dependents(chain.creator)
            .await
            .expect("the records answer")
            == vec![chain.variant.hash()],
        "the variant was not found under its creator",
    );
    checked.wants(
        "a variant depends on the message it carries",
        inverse
            .message_dependents(chain.message)
            .await
            .expect("the records answer")
            == vec![chain.variant.hash()],
        "the variant was not found under its message",
    );
    checked.wants(
        "the first version is numbered from nothing",
        inverse
            .version_num(chain.version.hash())
            .await
            .expect("the records answer")
            == 0,
        "the first version was not 0",
    );
    checked.wants(
        "the root is numbered one before the first version",
        inverse
            .version_num(chain.root)
            .await
            .expect("the records answer")
            == ROOT_VERSION,
        "the root was not -1",
    );
}

/// An object written after a rebuild is answered for all the same, by the objects themselves.
async fn a_write_the_records_do_not_describe_falls_back(sandbox: &Sandbox, checked: &mut Checked) {
    let index = index(sandbox, "stale");
    let chain = chain(&index).await;
    let inverse = inverse(&index);
    inverse.rebuild().await.expect("the inverse index is built");

    // One more variant on top of the version, written after the records were made: what they
    // describe is now one object short of what the index holds.
    let next = chain
        .version
        .new_variant([0x22; 32], [0xaa; 32], [0xbb; 32]);
    index
        .write(next.clone())
        .await
        .expect("the variant is written");

    checked.wants(
        "a dependency the records do not hold is answered from the objects",
        inverse
            .variant_dependents(next.hash())
            .await
            .expect("the objects answer")
            == Vec::<Key>::new(),
        "the version that does not exist yet was answered for",
    );
    checked.wants(
        "a variant written after the rebuild is found under its base version",
        inverse
            .version_dependents(chain.version.hash())
            .await
            .expect("the objects answer")
            == vec![next.hash()],
        "the later variant was not found",
    );
    checked.wants(
        "a variant written after the rebuild is found under its storage address",
        inverse
            .store_dependents(Key::new([0x22; 32]))
            .await
            .expect("the objects answer")
            == vec![next.hash()],
        "the later variant was not found under its storage address",
    );
}

/// A rebuild after more objects takes them in: the records grow to describe the index as it is.
async fn a_rebuild_takes_in_what_was_written_after(sandbox: &Sandbox, checked: &mut Checked) {
    let index = index(sandbox, "again");
    let chain = chain(&index).await;
    let inverse = inverse(&index);
    inverse.rebuild().await.expect("the inverse index is built");

    let next = chain
        .version
        .new_variant([0x22; 32], [0xaa; 32], [0xbb; 32]);
    index
        .write(next.clone())
        .await
        .expect("the variant is written");

    let report = inverse
        .rebuild()
        .await
        .expect("the inverse index is built again");
    checked.wants(
        "a rebuild takes in the objects written since the last one",
        report.objects == 4,
        &format!("{} objects were read", report.objects),
    );
    checked.wants(
        "what was answered by falling back is answered by the records",
        inverse
            .version_dependents(chain.version.hash())
            .await
            .expect("the records answer")
            == vec![next.hash()],
        "the later variant was not found after the second rebuild",
    );
}

/// Packing the objects leaves every answer where it was: the records name keys, not places.
async fn a_pack_does_not_change_what_is_answered(sandbox: &Sandbox, checked: &mut Checked) {
    let index = index(sandbox, "packed");
    let chain = chain(&index).await;
    let inverse = inverse(&index);
    inverse.rebuild().await.expect("the inverse index is built");

    index.repack().await.expect("the index is packed");

    checked.wants(
        "an answer from the records survives packing",
        inverse
            .store_dependents(chain.storage)
            .await
            .expect("the records answer")
            == vec![chain.variant.hash()],
        "the records stopped answering once the objects were packed",
    );
    checked.wants(
        "a number from the records survives packing",
        inverse
            .version_num(chain.version.hash())
            .await
            .expect("the records answer")
            == 0,
        "the number stopped being answered once the objects were packed",
    );
}

/// Nothing stored under a key is answered for by nothing, whether from the records or the objects.
async fn nothing_is_answered_as_stored_when_nothing_is_there(
    sandbox: &Sandbox,
    checked: &mut Checked,
) {
    let index = index(sandbox, "empty");
    let inverse = inverse(&index);

    checked.wants(
        "nothing depends on a key nothing is stored under",
        inverse
            .store_dependents(Key::new([0x77; 32]))
            .await
            .expect("the objects answer")
            == Vec::<Key>::new(),
        "a dependency was found for a key nothing is stored under",
    );
}

/// What was asked of the inverse index, and how many of the questions were answered as they should
/// be.
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
