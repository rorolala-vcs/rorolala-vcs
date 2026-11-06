//! `inverse-index`: what the reverse dependency index answers, and what it falls back to.
//!
//! The index is content addressed, so it answers "what is stored under this hash" and nothing the
//! other way. The inverse index is the other way, which objects depend on a key, kept as records
//! beside the objects, and built by reading every object once. It is derived data, so what matters
//! is that an answer is the same whether it came from the records or from the objects, that it is
//! right the moment an object is written the records do not describe, and that what is answered does
//! not change when the objects are packed.
//!
//! A program rather than a set of tests cargo runs, for the same reason the rest of them are.

use librorolala::inverse_index::{GraphEdgeKind, GraphRequest, InverseIndex, RebuildMode};
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
    a_number_fills_when_the_link_it_waited_for_arrives(&sandbox, &mut checked).await;
    a_snapshot_says_what_the_records_say(&sandbox, &mut checked).await;
    a_graph_window_walks_the_chain(&sandbox, &mut checked).await;
    a_merge_is_answered_and_walked(&sandbox, &mut checked).await;
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
        "a first rebuild reads every object the index holds",
        report.mode == RebuildMode::Full && report.objects_read == 3,
        &format!(
            "a {} rebuild read {} of {} objects",
            report.mode.name(),
            report.objects_read,
            report.objects
        ),
    );
    checked.wants(
        "a first rebuild numbers the root and the version made of it",
        report.versions_numbered == 2,
        &format!("{} versions were numbered", report.versions_numbered),
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
        "a rebuild after a write is incremental",
        report.mode == RebuildMode::Incremental,
        &format!("a {} rebuild ran", report.mode.name()),
    );
    checked.wants(
        "an incremental rebuild reads only what is new",
        report.objects_read == 1,
        &format!("{} objects were read", report.objects_read),
    );
    checked.wants(
        "an incremental rebuild still describes every object",
        report.objects == 4,
        &format!("{} objects are held", report.objects),
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

    // Nothing was written since: a rebuild has nothing to do and says so rather than reading again.
    let report = inverse
        .rebuild()
        .await
        .expect("the inverse index is built again");
    checked.wants(
        "a rebuild with nothing new is fresh",
        report.mode == RebuildMode::Fresh,
        &format!("a {} rebuild ran", report.mode.name()),
    );
    checked.wants(
        "a fresh rebuild reads nothing",
        report.objects_read == 0,
        &format!("{} objects were read", report.objects_read),
    );
}

/// A version whose base was not written yet is numbered by the rebuild that brings the base in.
async fn a_number_fills_when_the_link_it_waited_for_arrives(
    sandbox: &Sandbox,
    checked: &mut Checked,
) {
    let index = index(sandbox, "numbering");
    let inverse = inverse(&index);

    // The root and the first variant are made but not written: the second version is based on the
    // first, so what is written is a chain with its bottom end missing.
    let root = Version::root();
    let first_variant = root.new_variant([0x11; 32], [0xaa; 32], [0xbb; 32]);
    let first = first_variant.new_version();
    let second_variant = first.new_variant([0x22; 32], [0xaa; 32], [0xbb; 32]);
    let second = second_variant.new_version();

    index
        .write(second_variant.clone())
        .await
        .expect("the variant is written");
    index
        .write(second.clone())
        .await
        .expect("the version is written");

    let report = inverse.rebuild().await.expect("the inverse index is built");
    checked.wants(
        "a version cut off from the root is not numbered",
        report.versions_numbered == 0,
        &format!("{} versions were numbered", report.versions_numbered),
    );

    // The bottom of the chain arrives: the next rebuild numbers the versions from the root.
    index
        .write(root.clone())
        .await
        .expect("the root is written");
    index
        .write(first_variant.clone())
        .await
        .expect("the first variant is written");
    index
        .write(first.clone())
        .await
        .expect("the first version is written");

    let report = inverse
        .rebuild()
        .await
        .expect("the inverse index is built again");
    checked.wants(
        "the rebuild that fills the hole is incremental",
        report.mode == RebuildMode::Incremental,
        &format!("a {} rebuild ran", report.mode.name()),
    );
    checked.wants(
        "a number waiting for its base is filled, the root's own included",
        report.versions_numbered == 3,
        &format!("{} versions were numbered", report.versions_numbered),
    );
    checked.wants(
        "the versions are numbered from the root outwards",
        inverse
            .version_num(first.hash())
            .await
            .expect("the records answer")
            == 0
            && inverse
                .version_num(second.hash())
                .await
                .expect("the records answer")
                == 1,
        "the versions were not numbered 0 and 1",
    );
}

/// A snapshot is the records read the other way: what each variant is made of, and its number.
async fn a_snapshot_says_what_the_records_say(sandbox: &Sandbox, checked: &mut Checked) {
    let index = index(sandbox, "snapshot");
    let chain = chain(&index).await;
    let inverse = inverse(&index);

    checked.wants(
        "a snapshot is nothing while the records do not describe the index",
        inverse
            .snapshot()
            .await
            .expect("the records read")
            .is_none(),
        "the records that were never built answered for a snapshot",
    );

    inverse.rebuild().await.expect("the inverse index is built");

    let snapshot = inverse
        .snapshot()
        .await
        .expect("the records read")
        .expect("the records describe the index");

    checked.wants(
        "a snapshot names the variant the records hold",
        snapshot.variants().count() == 1 && snapshot.variant(chain.variant.hash()).is_some(),
        &format!("{} variants were named", snapshot.variants().count()),
    );
    checked.wants(
        "a snapshot names the versions the records hold",
        snapshot.versions().count() == 2,
        &format!("{} versions were named", snapshot.versions().count()),
    );
    checked.wants(
        "a variant's base is the version it was made from",
        snapshot
            .variant(chain.variant.hash())
            .is_some_and(|record| record.base_version == chain.root),
        "the variant named another base",
    );
    checked.wants(
        "the versions fixing a variant are found from it",
        snapshot.versions_pinning(chain.variant.hash()) == vec![chain.version.hash()],
        "the version was not found under its variant",
    );
    checked.wants(
        "the first version is numbered from nothing",
        snapshot
            .version(chain.version.hash())
            .is_some_and(|record| record.number == Some(0) && !record.is_root),
        "the first version did not read as numbered 0",
    );
    checked.wants(
        "the root is named as the root and not numbered",
        snapshot
            .version(chain.root)
            .is_some_and(|record| record.is_root && record.number.is_none()),
        "the root did not read as the root",
    );

    // A variant written since makes the records older than the index: they answer no snapshot, and
    // what the index holds is read instead.
    let later = chain
        .version
        .new_variant([0x22; 32], [0xaa; 32], [0xbb; 32]);
    index
        .write(later.clone())
        .await
        .expect("the variant is written");

    checked.wants(
        "a write the records do not describe leaves no snapshot",
        inverse
            .snapshot()
            .await
            .expect("the records read")
            .is_none(),
        "the stale records answered for a snapshot",
    );

    let scanned = inverse.scan().await.expect("the objects answer");
    checked.wants(
        "a scan answers with what the objects hold",
        scanned.variants().count() == 2 && scanned.variant(later.hash()).is_some(),
        &format!("{} variants were read", scanned.variants().count()),
    );
}

/// A graph window is cut around a focus: the chain behind it, and where the view stops.
async fn a_graph_window_walks_the_chain(sandbox: &Sandbox, checked: &mut Checked) {
    let index = index(sandbox, "graph");
    let chain = chain(&index).await;
    let inverse = inverse(&index);
    inverse.rebuild().await.expect("the inverse index is built");

    let snapshot = inverse
        .snapshot()
        .await
        .expect("the records read")
        .expect("the records describe the index");
    let view = snapshot.graph(&GraphRequest {
        focus: vec![chain.version.hash()],
        ..GraphRequest::default()
    });

    checked.wants(
        "a window is anchored on the variant the focus fixes",
        view.anchors.len() == 1
            && view.anchors[0].version == chain.version.hash()
            && view.anchors[0].variant == Some(chain.variant.hash()),
        "the anchor did not resolve to the variant",
    );

    let node = view
        .nodes
        .iter()
        .find(|node| node.hash == chain.variant.hash());
    checked.wants(
        "the chain stops at the root rather than at a variant",
        node.is_some_and(|node| {
            node.base_version == chain.root && node.base_variant.is_none() && node.frontier.parents
        }),
        "the variant's base did not read as the root",
    );
    checked.wants(
        "the versions of the window carry their numbers",
        view.versions
            .iter()
            .any(|version| version.hash == chain.version.hash() && version.number == Some(0))
            && view.versions.iter().any(|version| {
                version.hash == chain.root && version.is_root && version.number.is_none()
            }),
        "the numbers in the window were not the chain's",
    );
    checked.wants(
        "a chain whose root is in the window names nothing missing",
        view.missing.is_empty() && !view.truncated,
        "the window named a missing key or was cut short",
    );
}

/// A merge is found from the variant it merges in, and a window follows it.
async fn a_merge_is_answered_and_walked(sandbox: &Sandbox, checked: &mut Checked) {
    let index = index(sandbox, "merge");
    let chain = chain(&index).await;
    let inverse = inverse(&index);

    let merge = Variant::new_bare_variant(
        [0x33; 32],
        *chain.version.hash().digest(),
        Some(*chain.variant.hash().digest()),
        [0xaa; 32],
        [0xbb; 32],
        0,
    );
    let merge_version = merge.new_version();

    index
        .write(merge.clone())
        .await
        .expect("the merge is written");
    index
        .write(merge_version.clone())
        .await
        .expect("the version is written");

    inverse.rebuild().await.expect("the inverse index is built");

    checked.wants(
        "a merge is found from the variant it merges in",
        inverse
            .join_dependents(chain.variant.hash())
            .await
            .expect("the records answer")
            == vec![merge.hash()],
        "the merge was not found under the variant it joins",
    );

    let snapshot = inverse
        .snapshot()
        .await
        .expect("the records read")
        .expect("the records describe the index");
    checked.wants(
        "the records say which variants merge one in",
        snapshot.variants_joining(chain.variant.hash()) == vec![merge.hash()],
        "the merge was not named as joining",
    );

    let view = snapshot.graph(&GraphRequest {
        focus: vec![merge_version.hash()],
        ..GraphRequest::default()
    });
    checked.wants(
        "a window follows a merge into the chain it merges",
        view.nodes.iter().any(|node| node.hash == merge.hash())
            && view
                .nodes
                .iter()
                .any(|node| node.hash == chain.variant.hash())
            && view.edges.iter().any(|edge| {
                edge.from == merge.hash()
                    && edge.to == chain.variant.hash()
                    && edge.kind == GraphEdgeKind::Join
            }),
        "the merge's edge was not in the window",
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
            self.wrong.push(format!("{question}, {said}"));
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
