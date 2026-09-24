//! `storage-packing`: what a store answers once its objects are packed.
//!
//! A pack is a place objects live rather than a thing a caller knows about: an object that moves
//! into one is the same bytes under the same key, and every read that worked loose still works
//! packed. That is the whole promise, and this is where it is asked of a store that has actually
//! been packed — not of an index in isolation.
//!
//! A program rather than a set of tests cargo runs, for the same reason the rest of them are: what
//! it does is what a person does with a terminal, and what it checks is what a program would say
//! back.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use librorolala::storage::{
    AlgorithmChoice, Chunk, Chunking, Codec, Error, FRAME_MAGIC, Key, Lockable as _, PackEntry,
    PackIndex, RorolalaStorage, StorageBackend as _, internals::Internals as _,
};
use rorolala_utils_sandbox::Sandbox;

#[tokio::main]
async fn main() {
    let sandbox = Sandbox::new("storage-packing");
    let mut checked = Checked::default();

    packing_moves_objects_into_a_pack(&sandbox, &mut checked).await;
    packing_leaves_the_pack_as_one_run_of_entries(&sandbox, &mut checked).await;
    packing_what_is_packed_already_makes_no_pack(&sandbox, &mut checked).await;
    a_store_being_packed_is_not_packed_again(&sandbox, &mut checked).await;
    a_number_already_taken_is_passed_over(&sandbox, &mut checked).await;
    a_key_a_pack_names_is_not_packed_again(&sandbox, &mut checked).await;
    a_key_packed_at_once_is_taken_from_every_pack(&sandbox, &mut checked).await;
    two_removes_at_once_both_take_effect(&sandbox, &mut checked).await;
    an_index_that_does_not_read_hides_only_its_own_pack(&sandbox, &mut checked).await;
    an_index_that_is_gone_hides_only_its_own_pack(&sandbox, &mut checked).await;
    a_pack_whose_index_does_not_read_is_reported(&sandbox, &mut checked).await;
    a_pack_that_changes_is_read_afresh(&sandbox, &mut checked).await;
    a_pack_changed_by_another_writer_is_read_afresh(&sandbox, &mut checked).await;
    a_number_given_back_is_not_answered_with_the_pack_that_had_it(&sandbox, &mut checked).await;
    a_change_made_through_another_store_is_seen(&sandbox, &mut checked).await;
    a_content_whose_chunks_were_packed_is_still_whole(&sandbox, &mut checked).await;
    a_packed_object_can_be_dropped(&sandbox, &mut checked).await;
    dropping_a_packed_object_leaves_the_pack_bytes_where_they_are(&sandbox, &mut checked).await;
    a_pack_left_holding_nothing_is_dropped_whole(&sandbox, &mut checked).await;
    repacking_merges_the_packs_there_are(&sandbox, &mut checked).await;
    repacking_rolls_over_at_the_size_the_store_allows(&sandbox, &mut checked).await;
    repacking_leaves_a_store_already_laid_out_alone(&sandbox, &mut checked).await;
    repacking_leaves_a_pack_that_does_not_read_where_it_is(&sandbox, &mut checked).await;
    a_manifest_is_packed_with_the_manifests(&sandbox, &mut checked).await;
    packing_leaves_no_empty_directories(&sandbox, &mut checked).await;
    repacking_numbers_the_packs_from_nothing(&sandbox, &mut checked).await;
    rewriting_packed_content_leaves_it_packed(&sandbox, &mut checked).await;

    sandbox.cleanup();
    checked.report();
}

/// Packing a batch of objects moves them into a pack, and reads answer exactly as they did loose.
async fn packing_moves_objects_into_a_pack(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "moves");
    let contents = contents(16, 512);
    let mut keys = Vec::new();

    for content in &contents {
        keys.push(written(&store, content).await);
    }

    let before = store.list_all_keys().await.expect("the store lists");
    checked.wants(
        "the objects are loose to begin with",
        objects(sandbox, "moves") == keys.len(),
        "the objects were not all loose",
    );

    let made = store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&keys)
        .await
        .expect("the objects are packed");
    checked.wants("packing a batch makes a pack", made, &format!("{made}"));
    checked.wants(
        "the pack it made is the first",
        store.packs().await.unwrap_or_default() == [0],
        "the pack was not the first",
    );

    checked.wants(
        "nothing is left loose once it is packed",
        objects(sandbox, "moves") == 0,
        "an object was still loose after it was packed",
    );

    let (pack_path, index_path) = store.pack_paths(0);
    checked.wants(
        "the pack and the index beside it are there",
        pack_path.is_file() && index_path.is_file(),
        "the pack or its index is not there",
    );

    let directory = store.read_pack_index(0).await.expect("the index is read");
    checked.wants(
        "the pack holds every object it was given",
        directory.len() == keys.len(),
        &format!("{} entries for {} keys", directory.len(), keys.len()),
    );

    let after = store.list_all_keys().await.expect("the store lists");
    checked.wants(
        "a packed store lists what a loose one did",
        after == before,
        "the keys changed when they were packed",
    );

    let mut reads_back = true;
    for (key, content) in keys.iter().zip(&contents) {
        reads_back &= holds(&store, key).await;
        reads_back &= store
            .read_object(key)
            .await
            .is_ok_and(|read| &read == content);
    }

    checked.wants(
        "every packed object reads back as it was written",
        reads_back,
        "a packed object did not read back",
    );
}

/// The pack file is the entries one after another, each beginning with the frame it had loose.
async fn packing_leaves_the_pack_as_one_run_of_entries(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "run");
    let mut keys = Vec::new();

    for content in &contents(9, 700) {
        keys.push(written(&store, content).await);
    }

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&keys)
        .await
        .expect("the objects are packed");

    let (pack_path, _) = store.pack_paths(0);
    let pack = fs::read(&pack_path).expect("the pack is read");
    let directory = store.read_pack_index(0).await.expect("the index is read");

    let mut right_after = true;
    let mut framed = true;
    let mut at = 0_u64;

    for entry in directory.entries() {
        right_after &= entry.offset() == at;

        let start = usize::try_from(entry.offset()).expect("an offset fits a machine");
        let end = start + usize::try_from(entry.len()).expect("a length fits a machine");
        framed &= pack
            .get(start..end)
            .is_some_and(|bytes| bytes.starts_with(&FRAME_MAGIC));

        at = entry.offset() + entry.len();
    }

    checked.wants(
        "each entry sits right after the one before it",
        right_after,
        "the pack has a gap or an overlap",
    );
    checked.wants(
        "every entry begins with the frame it had loose",
        framed,
        "an entry in the pack does not begin with a frame",
    );
    checked.wants(
        "the entries are the whole of the pack",
        at == pack.len() as u64,
        &format!("the entries cover {at} of {} bytes", pack.len()),
    );
}

/// Packing what is packed already makes no pack, and the next batch gets the next number.
async fn packing_what_is_packed_already_makes_no_pack(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "again");
    let mut keys = Vec::new();

    for content in &contents(4, 256) {
        keys.push(written(&store, content).await);
    }

    let first = store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&keys)
        .await
        .expect("the objects are packed");
    let again = store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&keys)
        .await
        .expect("the objects are packed again");

    checked.wants("the first batch is a pack", first, &format!("{first}"));
    checked.wants(
        "packing what is packed already is no pack",
        !again,
        &format!("{again}"),
    );

    let next = written(&store, b"a later object").await;
    let second = store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&[next])
        .await
        .expect("the later object is packed");

    checked.wants("the second batch is a pack", second, &format!("{second}"));
    checked.wants(
        "the pack after one is the next pack",
        store.packs().await.unwrap_or_default() == [0, 1],
        "the second pack was not the next one",
    );
}

/// A store being packed is not packed again: a run that finds the lock taken is told so.
///
/// A pack is read, changed and written back, so two runs at one store would each write a pack that
/// had forgotten the other's — and the losing batch's loose copies are gone by the time its pack is
/// overwritten, so nothing would hold it. Keeping them apart is what the lock is for, and it is one
/// file at the store's root rather than one per opener: a second store over the same directory is
/// turned away just the same. Where two packings were once raced for two numbers, what is asked now
/// is the lock itself — that a run which would rather not wait is told to come back, and that the
/// store is packable again once the guard goes.
async fn a_store_being_packed_is_not_packed_again(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "being-packed");
    let key = written(&store, b"the batch").await;

    let guard = store.lock().await.expect("the store is free to lock");

    checked.wants(
        "a store being packed is not locked again",
        store.lock().await.is_err(),
        "the store was locked twice",
    );

    // The lock is one file at the root rather than one per opener, so a store made the same way over
    // the same directory is the same place, and is refused the same.
    let other = RorolalaStorage::at(sandbox.join("being-packed"))
        .expect("the store made a moment ago is found");
    checked.wants(
        "another store of one directory is not locked either",
        other.lock().await.is_err(),
        "another store over one directory took the lock",
    );
    checked.wants(
        "a store being packed says it is locked",
        store.is_locking(),
        "the store did not say it was locked",
    );

    drop(guard);

    checked.wants(
        "a store whose guard has gone is not locked",
        !store.is_locking(),
        "the store still says it is locked after the guard went",
    );

    let made = store
        .lock()
        .await
        .expect("the lock is free once the guard has gone")
        .pack(&[key])
        .await
        .expect("the object is packed");

    checked.wants(
        "the batch is packed once the lock is free",
        made,
        &format!("{made}"),
    );
    checked.wants(
        "the object packed after the lock was given back reads back",
        store
            .read_object(&key)
            .await
            .is_ok_and(|read| read == b"the batch"),
        "the packed object did not read back",
    );
}

/// A number whose pack file is already there is passed over, even with no index beside it.
async fn a_number_already_taken_is_passed_over(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "taken");

    // An orphan pack left by a write that was cut short: the file is there, the index is not, so
    // no reader reaches it — but the number is not free to take either.
    let (orphan, _) = store.pack_paths(0);
    fs::write(&orphan, b"an orphan pack").expect("the orphan pack is written");

    let key = written(&store, b"the object").await;
    let made = store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&[key])
        .await
        .expect("the object is packed");

    checked.wants("the object is packed", made, &format!("{made}"));
    checked.wants(
        "a number with a pack file already is passed over",
        store.packs().await.unwrap_or_default() == [1],
        "the number with a pack file already was taken again",
    );
    checked.wants(
        "the orphan pack is left where it was",
        fs::read(&orphan).is_ok_and(|bytes| bytes == b"an orphan pack"),
        "the orphan pack was written over",
    );
    checked.wants(
        "the object packed under the free number reads back",
        store
            .read_object(&key)
            .await
            .is_ok_and(|read| read == b"the object"),
        "the packed object did not read back",
    );
}

/// A key a pack already names is not packed a second time, even if a loose copy is back.
///
/// The loose copy coming back is what a write cut short leaves behind: the index names the object and
/// the object is loose as well, because the loose copies are dropped last. Packing that key again
/// would put it in two packs, and a key in two packs is one `remove` cannot take away by name alone.
async fn a_key_a_pack_names_is_not_packed_again(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "named-already");
    let content = b"the object";
    let key = written(&store, content).await;

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&[key])
        .await
        .expect("the object is packed");

    // The loose copy is written again, as a run that stopped before dropping it would leave it.
    let _again = written(&store, content).await;
    let packed = store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&[key])
        .await
        .expect("the object is packed again");

    checked.wants(
        "a key in a pack is not packed a second time",
        !packed,
        &format!("{packed}"),
    );
    checked.wants(
        "the store holds one pack, not two",
        store.packs().await.unwrap_or_default().len() == 1,
        "a second pack was made for a key that was in one already",
    );
}

/// A key that ended up in more than one pack is taken out of all of them by one `remove`.
///
/// Nothing keeps a key out of two packs when the two are written at once, so `remove` is what has to
/// hold the contract shut: a key it says it dropped must not still be readable from a pack it did not
/// think to look in. The second pack is built by hand rather than raced for, so what is being asked
/// does not depend on how two tasks happen to be scheduled.
async fn a_key_packed_at_once_is_taken_from_every_pack(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "every-pack");
    let key = written(&store, b"the object").await;
    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&[key])
        .await
        .expect("the object is packed");

    // A second pack naming the same key, as two packs written for it at once would leave.
    let directory = store
        .read_pack_index(0)
        .await
        .expect("the first index is read");
    let entry = *directory.find(&key).expect("the first pack names the key");
    let (first_pack, _) = store.pack_paths(0);
    let first_bytes = fs::read(&first_pack).expect("the first pack is read");
    let start = usize::try_from(entry.offset()).expect("an offset fits a machine");
    let end = start + usize::try_from(entry.len()).expect("a length fits a machine");

    let (second_pack, second_index) = store.pack_paths(1);
    fs::write(&second_pack, &first_bytes[start..end]).expect("the second pack is written");
    let mut second = PackIndex::default();
    second.push(PackEntry::new(key, 0, entry.len()));
    fs::write(&second_index, second.encode()).expect("the second index is written");

    checked.wants(
        "the key is in two packs to begin with",
        store
            .read_pack_index(0)
            .await
            .is_ok_and(|index| index.find(&key).is_some())
            && store
                .read_pack_index(1)
                .await
                .is_ok_and(|index| index.find(&key).is_some()),
        "the key was not made to sit in two packs",
    );

    store.remove(&key).await.expect("the object is dropped");

    checked.wants(
        "a key removed from a store is not held anywhere",
        !holds(&store, &key).await,
        "the key is still held after it was dropped",
    );
    checked.wants(
        "a key removed from a store does not read back",
        matches!(store.read_object(&key).await, Err(Error::NotFound(_))),
        "the key still read back after it was dropped",
    );
    checked.wants(
        "a key removed from a store is not listed",
        !store
            .list_all_keys()
            .await
            .unwrap_or_default()
            .contains(&key),
        "the key is still listed after it was dropped",
    );
    checked.wants(
        "both packs that named the key let it go",
        store.packs().await.unwrap_or_default().is_empty(),
        "a pack still names the key that was dropped",
    );
}

/// Two objects dropped from one pack at once are both dropped.
///
/// A pack's index is read, changed and written back, so two removals that do not take turns each
/// write an index that has forgotten the other's change — and one of the two objects the store said
/// it had dropped is still named. Changing packs is done under one lock, and the lock is one *per
/// root* rather than one per store, so the two removals are made through two stores of one directory:
/// a store is handed out freely, and a lock each store keeps to itself would not make them take
/// turns.
async fn two_removes_at_once_both_take_effect(sandbox: &Sandbox, checked: &mut Checked) {
    let root = sandbox.join("removes-at-once");
    let store = RorolalaStorage::create(&root);
    let other = RorolalaStorage::at(&root).expect("the store made a moment ago is found");
    let contents = contents(6, 200);
    let mut keys = Vec::new();

    for content in &contents {
        keys.push(written(&store, content).await);
    }

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&keys)
        .await
        .expect("the objects are packed");
    let (first, second) = (keys[1], keys[4]);

    let (left, right) = tokio::join!(store.remove(&first), other.remove(&second));
    left.expect("the first object is dropped");
    right.expect("the second object is dropped");

    let mut both_gone = true;
    for gone in [first, second] {
        both_gone &= !holds(&store, &gone).await;
        both_gone &= matches!(store.read_object(&gone).await, Err(Error::NotFound(_)));
        both_gone &= !store
            .list_all_keys()
            .await
            .unwrap_or_default()
            .contains(&gone);
    }

    checked.wants(
        "two removals at once both take effect",
        both_gone,
        "an object dropped from a pack is still there",
    );
    checked.wants(
        "the index has let both of them go",
        store
            .read_pack_index(0)
            .await
            .is_ok_and(|index| index.len() == keys.len() - 2),
        "the pack index kept an object that was dropped",
    );

    let mut rest = true;
    for (key, content) in keys.iter().zip(&contents) {
        if *key == first || *key == second {
            continue;
        }

        rest &= store
            .read_object(key)
            .await
            .is_ok_and(|read| &read == content);
    }

    checked.wants(
        "the objects that were not dropped still read back",
        rest,
        "another packed object stopped reading back",
    );
}

/// A pack whose index does not read is reported, so that a store going bad is not silent.
async fn a_pack_whose_index_does_not_read_is_reported(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "reported");
    let key = written(&store, b"the object").await;
    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&[key])
        .await
        .expect("the object is packed");

    checked.wants(
        "a store that holds together reports nothing broken",
        store.broken_packs().await.unwrap_or_default().is_empty(),
        "a pack was reported broken in a store that is whole",
    );

    let (_, index_path) = store.pack_paths(0);
    fs::write(&index_path, b"not an index at all").expect("the index is overwritten");

    checked.wants(
        "a pack whose index does not read is reported",
        store.broken_packs().await.unwrap_or_default() == [0],
        "the unreadable index was not reported",
    );
}

/// A pack that changes after it has been read is read afresh, not answered from what was read before.
///
/// What an index says is remembered to save reading it again for every object a lookup walks past,
/// and the danger of remembering is answering an object that has since been dropped: the bytes of a
/// dropped object stay in the pack, so an answer from a stale index would hand them back.
async fn a_pack_that_changes_is_read_afresh(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "changes");
    let contents = contents(3, 300);
    let mut keys = Vec::new();

    for content in &contents {
        keys.push(written(&store, content).await);
    }

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&keys)
        .await
        .expect("the objects are packed");

    // Reading one warms whatever was remembered of the pack.
    checked.wants(
        "a packed object reads back before anything changes",
        store.read_object(&keys[0]).await.is_ok(),
        "the packed object did not read back",
    );

    store.remove(&keys[1]).await.expect("the object is dropped");

    checked.wants(
        "an object dropped after the pack was read is not handed back",
        matches!(store.read_object(&keys[1]).await, Err(Error::NotFound(_))),
        "the dropped object was still handed back",
    );
    checked.wants(
        "what the store holds follows what it was last told",
        !holds(&store, &keys[1]).await,
        "the store still holds an object it dropped",
    );
    checked.wants(
        "the objects that were not dropped still read back",
        store
            .read_object(&keys[0])
            .await
            .is_ok_and(|read| read == contents[0]),
        "an object that was not dropped stopped reading back",
    );
}

/// A pack another writer changed is read afresh, not answered from what this process read before.
async fn a_pack_changed_by_another_writer_is_read_afresh(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "other-writer");
    let first = written(&store, b"the object that is kept").await;
    let second = written(&store, b"the object another writer drops").await;

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&[first, second])
        .await
        .expect("the objects are packed");

    checked.wants(
        "a packed object reads back before another writer acts",
        store.read_object(&second).await.is_ok(),
        "the packed object did not read back",
    );

    // Another process writing an index the way this store does: fewer entries, whole file rewritten.
    let directory = store.read_pack_index(0).await.expect("the index is read");
    let kept = *directory.find(&first).expect("the pack names both objects");
    let (_, index_path) = store.pack_paths(0);
    let mut smaller = PackIndex::default();
    smaller.push(kept);
    fs::write(&index_path, smaller.encode()).expect("the index is rewritten");

    checked.wants(
        "an object gone from a pack another writer wrote is not handed back",
        matches!(store.read_object(&second).await, Err(Error::NotFound(_))),
        "the object another writer dropped was still handed back",
    );
    checked.wants(
        "the object the other writer kept still reads back",
        store.read_object(&first).await.is_ok(),
        "the kept object stopped reading back after another writer acted",
    );
}

/// A pack number given back by an emptied pack is not answered with what the last one held.
///
/// A number is given out as one past the highest there is, so emptying a pack frees its number for
/// the next packing — and an index read as the one before would name objects at offsets that no
/// longer mean them. What was read of a pack goes when the pack's index does.
async fn a_number_given_back_is_not_answered_with_the_pack_that_had_it(
    sandbox: &Sandbox,
    checked: &mut Checked,
) {
    let store = store(sandbox, "reused-number");
    let first = written(&store, b"the first object").await;

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&[first])
        .await
        .expect("the first object is packed");
    checked.wants(
        "the first object reads back before its pack is emptied",
        store.read_object(&first).await.is_ok(),
        "the first object did not read back",
    );

    // Emptying the pack takes its index away, which is what frees the number.
    store
        .remove(&first)
        .await
        .expect("the first object is dropped");

    let second = written(&store, b"the second object").await;
    let made = store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&[second])
        .await
        .expect("the second object is packed");

    checked.wants("the second object is packed", made, &format!("{made}"));
    checked.wants(
        "the number is given back to the next pack",
        store.packs().await.unwrap_or_default() == [0],
        "the number was not given back",
    );
    checked.wants(
        "the object packed under a number given back reads back",
        store
            .read_object(&second)
            .await
            .is_ok_and(|read| read == b"the second object"),
        "the object packing under a given-back number did not read back",
    );
    checked.wants(
        "the object the pack before it held is not handed back",
        matches!(store.read_object(&first).await, Err(Error::NotFound(_))),
        "the object of the pack that had the number was handed back",
    );
}

/// A change made through another store of one directory is seen by the first.
async fn a_change_made_through_another_store_is_seen(sandbox: &Sandbox, checked: &mut Checked) {
    let root = sandbox.join("one-directory");
    let store = RorolalaStorage::create(&root);
    let other = RorolalaStorage::at(&root).expect("the store made a moment ago is found");
    let contents = contents(3, 250);
    let mut keys = Vec::new();

    for content in &contents {
        keys.push(written(&store, content).await);
    }

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&keys)
        .await
        .expect("the objects are packed");
    checked.wants(
        "an object reads back before another store acts",
        store.read_object(&keys[0]).await.is_ok(),
        "the packed object did not read back",
    );

    other.remove(&keys[1]).await.expect("the object is dropped");

    checked.wants(
        "a store sees an object another store of one directory dropped",
        matches!(store.read_object(&keys[1]).await, Err(Error::NotFound(_))),
        "the dropped object was still handed back",
    );
    checked.wants(
        "a store still reads what another store of one directory left",
        store
            .read_object(&keys[0])
            .await
            .is_ok_and(|read| read == contents[0]),
        "an object that was not dropped stopped reading back",
    );
}

/// A pack whose index does not read is a pack nothing is found in — and no other pack is affected.
async fn an_index_that_does_not_read_hides_only_its_own_pack(
    sandbox: &Sandbox,
    checked: &mut Checked,
) {
    let store = store(sandbox, "bad-index");
    let lost = written(&store, b"the object whose index is gone").await;
    let kept = written(&store, b"the object that is kept").await;

    // The pack that is to go wrong is written first, so a read that finds the good pack is one that
    // had to walk past the bad one to get there.
    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&[lost])
        .await
        .expect("the first object is packed");
    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&[kept])
        .await
        .expect("the second object is packed");

    let (_, index_path) = store.pack_paths(0);
    fs::write(&index_path, b"not an index at all").expect("the index is overwritten");

    checked.wants(
        "an index that does not read does not hide a good pack",
        store.read_object(&kept).await.is_ok(),
        "a good pack stopped reading",
    );
    checked.wants(
        "a key whose index does not read is not found",
        matches!(store.read_object(&lost).await, Err(Error::NotFound(_))),
        "a key in an unreadable pack still read back",
    );
    checked.wants(
        "listing a store with an unreadable index does not fail",
        store
            .list_all_keys()
            .await
            .is_ok_and(|keys| keys.contains(&kept) && !keys.contains(&lost)),
        "the store could not be listed",
    );
    checked.wants(
        "holding is asked the same way reading is",
        holds(&store, &kept).await && !holds(&store, &lost).await,
        "what the store holds does not match what it reads",
    );
}

/// A pack whose index was taken away is a pack nothing is found in — and no other pack is affected.
async fn an_index_that_is_gone_hides_only_its_own_pack(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "gone-index");
    let lost = written(&store, b"the object whose index is gone").await;
    let kept = written(&store, b"the object that is kept").await;

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&[lost])
        .await
        .expect("the first object is packed");
    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&[kept])
        .await
        .expect("the second object is packed");

    // An index taken away between a scan and a read is what a `remove` of a pack's last object does.
    let (_, index_path) = store.pack_paths(0);
    fs::remove_file(&index_path).expect("the index is taken away");

    checked.wants(
        "an index that is gone does not hide a good pack",
        store.read_object(&kept).await.is_ok(),
        "a good pack stopped reading",
    );
    checked.wants(
        "a key whose index is gone is not found",
        matches!(store.read_object(&lost).await, Err(Error::NotFound(_))),
        "a key in a pack with no index still read back",
    );
    checked.wants(
        "listing a store with a missing index does not fail",
        store
            .list_all_keys()
            .await
            .is_ok_and(|keys| keys.contains(&kept) && !keys.contains(&lost)),
        "the store could not be listed",
    );
}

/// A cut content is still whole with every one of its chunks packed.
async fn a_content_whose_chunks_were_packed_is_still_whole(
    sandbox: &Sandbox,
    checked: &mut Checked,
) {
    let store = store(sandbox, "chunked");
    let content = repetitive(64 * 1024);
    let file = sandbox.join("chunked-content");
    fs::write(&file, &content).expect("the content is written to a file");

    let key = store
        .write_file(
            &file,
            AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 4 * 1024 }),
        )
        .await
        .expect("the cut content is written");

    let manifest = store
        .read_manifest(&key)
        .await
        .expect("the manifest is read")
        .expect("cut content has a manifest");
    let chunks: Vec<Key> = manifest.chunks().iter().map(|chunk| chunk.key()).collect();
    // Repetition makes the chunks of a file equal to one another, and one content is one object
    // however many chunks it fills, so what is loose is the distinct keys rather than the chunks.
    let distinct: BTreeSet<Key> = chunks.iter().copied().collect();

    checked.wants(
        "a cut content leaves its chunks loose",
        objects(sandbox, "chunked") == distinct.len(),
        &format!(
            "{} objects for {} distinct chunks",
            objects(sandbox, "chunked"),
            distinct.len()
        ),
    );

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&chunks)
        .await
        .expect("the chunks are packed");

    checked.wants(
        "packing the chunks leaves none of them loose",
        objects(sandbox, "chunked") == 0,
        "a chunk was still loose after it was packed",
    );

    let out = sandbox.join("chunked-out");
    let read = store.extract_file(&key, &out).await;

    checked.wants(
        "a cut content comes back whole with its chunks packed",
        read.is_ok() && fs::read(&out).is_ok_and(|bytes| bytes == content),
        &format!("{read:?}"),
    );

    // What the caller packs is keys; a key held as a manifest has no object of its own, so packing it
    // moves the manifest — into a pack of the manifests — and makes no object of it.
    let made = store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&[key])
        .await
        .expect("the manifest is packed");

    checked.wants(
        "packing a key held as a manifest packs the manifest",
        made,
        &format!("{made}"),
    );
    checked.wants(
        "a key held as a manifest is not packed as an object",
        matches!(store.read_object(&key).await, Err(Error::NotFound(_))),
        "the manifest was packed as an object",
    );

    let listed = store.list_all_keys().await.unwrap_or_default();
    checked.wants(
        "a packed manifest is still one of the store's keys",
        listed.contains(&key),
        "the key stopped being listed",
    );

    let out = sandbox.join("chunked-manifest-out");
    let read = store.extract_file(&key, &out).await;

    checked.wants(
        "the content comes back whole with its manifest packed",
        read.is_ok() && fs::read(&out).is_ok_and(|bytes| bytes == content),
        &format!("{read:?}"),
    );
}

/// A packed object can be dropped, and the index no longer names it.
async fn a_packed_object_can_be_dropped(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "dropped");
    let contents = contents(5, 300);
    let mut keys = Vec::new();

    for content in &contents {
        keys.push(written(&store, content).await);
    }

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&keys)
        .await
        .expect("the objects are packed");
    let gone = keys[2];

    store
        .remove(&gone)
        .await
        .expect("the packed object is dropped");

    checked.wants(
        "a dropped packed object is not held",
        !holds(&store, &gone).await,
        "the dropped object is still held",
    );
    checked.wants(
        "reading a dropped packed object says it is not there",
        matches!(store.read_object(&gone).await, Err(Error::NotFound(_))),
        "the dropped object still read back",
    );

    let listed = store.list_all_keys().await.expect("the store lists");
    checked.wants(
        "a dropped packed object is not listed",
        !listed.contains(&gone),
        "the dropped object is still listed",
    );

    let directory = store.read_pack_index(0).await.expect("the index is read");
    checked.wants(
        "the index no longer names the object that was dropped",
        directory.len() == keys.len() - 1,
        &format!("{} entries for {} keys", directory.len(), keys.len()),
    );

    let mut rest = true;
    for (key, content) in keys.iter().zip(&contents) {
        if *key == gone {
            continue;
        }

        rest &= store
            .read_object(key)
            .await
            .is_ok_and(|read| &read == content);
    }

    checked.wants(
        "dropping one packed object leaves the rest",
        rest,
        "another packed object stopped reading back",
    );
}

/// Dropping a packed object changes only the index; the pack's bytes stay where they are.
///
/// A pack and its index are two files, so a removal that rewrote the pack would leave a window where
/// the index describes a pack that is already another one. Only the index is rewritten — the entry is
/// dropped from it — and what the entry occupied is reclaimed by a later compaction rather than now.
async fn dropping_a_packed_object_leaves_the_pack_bytes_where_they_are(
    sandbox: &Sandbox,
    checked: &mut Checked,
) {
    let store = store(sandbox, "left-alone");
    let contents = contents(4, 400);
    let mut keys = Vec::new();

    for content in &contents {
        keys.push(written(&store, content).await);
    }

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&keys)
        .await
        .expect("the objects are packed");
    let (pack_path, _) = store.pack_paths(0);
    let before = fs::metadata(&pack_path).expect("the pack is there").len();

    store.remove(&keys[1]).await.expect("the object is dropped");

    let after = fs::metadata(&pack_path)
        .expect("the pack is still there")
        .len();
    checked.wants(
        "dropping a packed object leaves the pack bytes where they were",
        after == before,
        &format!("the pack went from {before} to {after} bytes"),
    );

    let mut rest = true;
    for (key, content) in keys.iter().zip(&contents) {
        if *key == keys[1] {
            continue;
        }

        rest &= store
            .read_object(key)
            .await
            .is_ok_and(|read| &read == content);
    }

    checked.wants(
        "the rest of the pack still reads back",
        rest,
        "another packed object stopped reading back",
    );
}

/// A pack holding one object is taken away with it, since a pack with no index is not a pack.
async fn a_pack_left_holding_nothing_is_dropped_whole(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "empty");
    let key = written(&store, b"the only object").await;

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&[key])
        .await
        .expect("the object is packed");
    store.remove(&key).await.expect("the object is dropped");

    let (pack_path, index_path) = store.pack_paths(0);
    checked.wants(
        "a pack holding nothing is taken away whole",
        !pack_path.exists() && !index_path.exists(),
        "the pack or its index is still there",
    );
    checked.wants(
        "a store with no packs says it has none",
        store.packs().await.unwrap_or_default().is_empty(),
        "a pack is still listed",
    );
}

/// Repacking gathers the packs there are into one, and reads answer as they did.
async fn repacking_merges_the_packs_there_are(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "merge");
    let contents = contents(6, 400);
    let mut keys = Vec::new();

    for content in &contents {
        keys.push(written(&store, content).await);
    }

    // Two batches, so the store holdings two packs is where it starts.
    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&keys[..3])
        .await
        .expect("the first batch is packed");
    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&keys[3..])
        .await
        .expect("the second batch is packed");

    checked.wants(
        "a store packed in two batches holds two packs",
        store.packs().await.unwrap_or_default().len() == 2,
        "the two batches did not make two packs",
    );

    let merged = store
        .lock()
        .await
        .expect("the store is free to lock")
        .repack()
        .await
        .expect("the packs are merged");
    checked.wants("repacking changes the store", merged, &format!("{merged}"));
    checked.wants(
        "the two packs become one",
        store.packs().await.unwrap_or_default().len() == 1,
        "the packs were not merged",
    );
    checked.wants(
        "nothing is left loose after repacking",
        objects(sandbox, "merge") == 0,
        "an object was left loose",
    );

    let mut reads_back = true;
    for (key, content) in keys.iter().zip(&contents) {
        reads_back &= store
            .read_object(key)
            .await
            .is_ok_and(|read| &read == content);
    }

    checked.wants(
        "every merged object reads back as it was written",
        reads_back,
        "a merged object did not read back",
    );
}

/// What one pack may hold is the store's to say, and packs roll over at it.
async fn repacking_rolls_over_at_the_size_the_store_allows(
    sandbox: &Sandbox,
    checked: &mut Checked,
) {
    let store = store(sandbox, "rollover");

    // A limit small enough that the objects cannot all sit in one pack, but well over one entry, so
    // packs of more than one entry are still possible.
    let limit = 2 * 1024;
    fs::write(store.config_path(), "[storage]\nmax_pack_size = \"2KiB\"\n")
        .expect("the configuration is written");

    let contents = contents(9, 512);
    let mut keys = Vec::new();
    for content in &contents {
        keys.push(written(&store, content).await);
    }

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .repack()
        .await
        .expect("the store is laid out");

    let packs = store.packs().await.unwrap_or_default();
    checked.wants(
        "a store over the limit is laid out as several packs",
        packs.len() > 1,
        &format!("{} packs", packs.len()),
    );

    // No pack grows past the limit, unless one entry is over it on its own — a limit cannot be met
    // by a single object that is bigger than it.
    let mut within = true;
    let mut reads_back = true;

    for index in &packs {
        let directory = store
            .read_pack_index(*index)
            .await
            .expect("the index is read");
        let total: u64 = directory.entries().iter().map(PackEntry::len).sum();

        within &= total <= limit || directory.len() == 1;
    }

    for (key, content) in keys.iter().zip(&contents) {
        reads_back &= store
            .read_object(key)
            .await
            .is_ok_and(|read| &read == content);
    }

    checked.wants(
        "no pack grows past the limit",
        within,
        "a pack was over the limit",
    );
    checked.wants(
        "every object over a rolled-over store reads back",
        reads_back,
        "an object over a rolled-over store did not read back",
    );
}

/// A store already laid out the way repacking would lay it out is left alone.
async fn repacking_leaves_a_store_already_laid_out_alone(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "steady");
    let contents = contents(4, 300);
    let mut keys = Vec::new();

    for content in &contents {
        keys.push(written(&store, content).await);
    }

    let first = store
        .lock()
        .await
        .expect("the store is free to lock")
        .repack()
        .await
        .expect("the store is laid out");
    let packs = store.packs().await.unwrap_or_default();
    let again = store
        .lock()
        .await
        .expect("the store is free to lock")
        .repack()
        .await
        .expect("the store is left alone");

    checked.wants(
        "laying a loose store out changes it",
        first,
        &format!("{first}"),
    );
    checked.wants(
        "a store already laid out is left alone",
        !again,
        &format!("{again}"),
    );
    checked.wants(
        "the packs are the ones it had",
        store.packs().await.unwrap_or_default() == packs,
        "the packs were written again",
    );

    let mut reads_back = true;
    for (key, content) in keys.iter().zip(&contents) {
        reads_back &= store
            .read_object(key)
            .await
            .is_ok_and(|read| &read == content);
    }

    checked.wants(
        "the objects read back after being left alone",
        reads_back,
        "an object stopped reading back",
    );
}

/// A pack whose index does not read is left where it is, since what it holds cannot be read again.
async fn repacking_leaves_a_pack_that_does_not_read_where_it_is(
    sandbox: &Sandbox,
    checked: &mut Checked,
) {
    let store = store(sandbox, "broken");
    let key = written(&store, b"an object a broken index names").await;

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&[key])
        .await
        .expect("the object is packed");

    let (_, index_path) = store.pack_paths(0);
    fs::write(&index_path, b"not an index").expect("the index is spoiled");

    let changed = store
        .lock()
        .await
        .expect("the store is free to lock")
        .repack()
        .await
        .expect("repacking runs");

    checked.wants(
        "repacking does nothing while a pack will not read",
        !changed,
        &format!("{changed}"),
    );
    checked.wants(
        "the pack that does not read is still there",
        store.packs().await.unwrap_or_default() == [0],
        "the pack that does not read was taken away",
    );
}

/// A manifest is packed like anything else, into a pack of its own kind.
async fn a_manifest_is_packed_with_the_manifests(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "manifest-pack");
    let content = repetitive(16 * 1024);
    let file = sandbox.join("manifest-pack-content");
    fs::write(&file, &content).expect("the content is written to a file");

    let key = store
        .write_file(
            &file,
            AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 1024 }),
        )
        .await
        .expect("the content is stored");

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .repack()
        .await
        .expect("the store is laid out");

    // Everything under the manifests' own directory is an index: the loose manifests are gone, and
    // what is left is where the manifests that were packed sit.
    let under_manifest = files(&sandbox.join("manifest-pack").join("manifest"));
    let indexes = under_manifest
        .iter()
        .filter(|path| path.extension().is_some_and(|kind| kind == "idx"))
        .count();
    let loose = under_manifest.len() - indexes;

    checked.wants(
        "a manifest is packed with the manifests",
        indexes > 0,
        &format!("{indexes} manifest indexes"),
    );
    checked.wants(
        "no manifest is left loose",
        loose == 0,
        &format!("{loose} loose manifests were left behind"),
    );
    checked.wants(
        "a packed manifest is still listed",
        store.list_manifest_keys().await.unwrap_or_default() == [key],
        "the packed manifest was not listed",
    );

    let out = sandbox.join("manifest-pack-out");
    checked.wants(
        "a packed manifest still puts the content back together",
        store
            .extract_file(&key, &out)
            .await
            .is_ok_and(|()| fs::read(&out).is_ok_and(|read| read == content)),
        "the content did not read back",
    );
}

/// Writing content whose chunks are packed writes nothing loose beside them.
async fn rewriting_packed_content_leaves_it_packed(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "rewrite");
    let content = repetitive(16 * 1024);
    let file = sandbox.join("rewrite-content");
    fs::write(&file, &content).expect("the content is written to a file");

    let choice = AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 1024 });
    let key = store
        .write_file(&file, choice)
        .await
        .expect("the content is stored");

    // The content is cut, so what it is stored as is chunks and a manifest naming them; packing the
    // chunks is what would leave a loose copy of each behind on a second write.
    let chunks: Vec<Key> = store
        .read_manifest(&key)
        .await
        .expect("the manifest reads")
        .expect("the content was cut")
        .chunks()
        .iter()
        .map(Chunk::key)
        .collect();
    store
        .lock()
        .await
        .expect("the store is free to lock")
        .pack(&chunks)
        .await
        .expect("the chunks are packed");

    let loose_before = objects(sandbox, "rewrite");
    let again = store
        .write_file(&file, choice)
        .await
        .expect("the content is stored again");
    let loose_after = objects(sandbox, "rewrite");

    checked.wants(
        "writing packed content again answers the same key",
        again == key,
        "the key changed",
    );
    checked.wants(
        "packing the chunks left nothing loose",
        loose_before == 0,
        &format!("{loose_before} loose after packing"),
    );
    checked.wants(
        "writing packed content again leaves nothing loose",
        loose_after == 0,
        &format!("{loose_after} loose after the second write"),
    );

    let out = sandbox.join("rewrite-out");
    checked.wants(
        "the content reads back whole after the second write",
        store
            .extract_file(&key, &out)
            .await
            .is_ok_and(|()| fs::read(&out).is_ok_and(|read| read == content)),
        "the content did not read back",
    );
}

/// A packed store is not left shaped like what it used to hold.
async fn packing_leaves_no_empty_directories(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "tidy");
    let contents = contents(6, 400);
    let mut keys = Vec::new();

    for content in &contents {
        keys.push(written(&store, content).await);
    }

    // Something cut, so the manifests' directory has a shape of its own to leave behind.
    let file = sandbox.join("tidy-content");
    let cut = repetitive(16 * 1024);
    fs::write(&file, &cut).expect("the content is written to a file");
    let chunked = store
        .write_file(
            &file,
            AlgorithmChoice::new(Codec::Raw, Chunking::Fixed { size: 1024 }),
        )
        .await
        .expect("the content is stored");

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .repack()
        .await
        .expect("the store is laid out");

    let root = sandbox.join("tidy");
    checked.wants(
        "a packed store keeps no empty directories",
        ["obj", "manifest"]
            .iter()
            .all(|name| directories_under(&root.join(name)).is_empty()),
        "a directory was left behind",
    );
    checked.wants(
        "the layout's own directories stay",
        root.join("obj").is_dir() && root.join("manifest").is_dir() && root.join("packed").is_dir(),
        "a layout directory was taken away",
    );

    let mut reads_back = true;
    for (key, content) in keys.iter().zip(&contents) {
        reads_back &= store
            .read_object(key)
            .await
            .is_ok_and(|read| &read == content);
    }

    let out = sandbox.join("tidy-out");
    reads_back &= store
        .extract_file(&chunked, &out)
        .await
        .is_ok_and(|()| fs::read(&out).is_ok_and(|read| read == cut));

    checked.wants(
        "nothing stopped reading back",
        reads_back,
        "something stopped reading back",
    );
}

/// Packing numbers the packs from nothing, without a gap, however often it runs.
async fn repacking_numbers_the_packs_from_nothing(sandbox: &Sandbox, checked: &mut Checked) {
    let store = store(sandbox, "numbers");

    // A limit small enough that one pack cannot hold everything.
    fs::write(store.config_path(), "[storage]\nmax_pack_size = \"2KiB\"\n")
        .expect("the configuration is written");

    let contents = contents(9, 512);
    let mut keys = Vec::new();
    for content in &contents {
        keys.push(written(&store, content).await);
    }

    store
        .lock()
        .await
        .expect("the store is free to lock")
        .repack()
        .await
        .expect("the store is laid out");
    let packs = store.packs().await.unwrap_or_default();

    checked.wants(
        "a store over the limit is laid out as several packs",
        packs.len() > 1,
        &format!("{} packs", packs.len()),
    );
    checked.wants(
        "the packs are numbered from nothing, without a gap",
        packs == (0..packs.len() as u64).collect::<Vec<u64>>(),
        &format!("{packs:?}"),
    );

    // Raising the limit merges them, and what is left starts over at nothing rather than being given
    // the number after the packs that are gone.
    fs::write(store.config_path(), "[storage]\nmax_pack_size = \"2GiB\"\n")
        .expect("the configuration is written");
    store
        .lock()
        .await
        .expect("the store is free to lock")
        .repack()
        .await
        .expect("the store is laid out again");
    let packs = store.packs().await.unwrap_or_default();

    checked.wants(
        "merging leaves one pack",
        packs.len() == 1,
        &format!("{} packs", packs.len()),
    );
    checked.wants(
        "the merged pack is numbered from nothing",
        packs == [0],
        &format!("{packs:?}"),
    );

    let mut reads_back = true;
    for (key, content) in keys.iter().zip(&contents) {
        reads_back &= store
            .read_object(key)
            .await
            .is_ok_and(|read| &read == content);
    }

    checked.wants(
        "everything still reads back",
        reads_back,
        "an object stopped reading back",
    );
}

/// The directories directly under `directory`.
fn directories_under(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };

    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}

/// A store of its own for one question, under `name` in the sandbox.
fn store(sandbox: &Sandbox, name: &str) -> RorolalaStorage {
    RorolalaStorage::create(sandbox.join(name))
}

/// Writes `content` as an object and answers with its key.
async fn written(store: &RorolalaStorage, content: &[u8]) -> Key {
    store
        .write_object(content, Codec::Raw)
        .await
        .expect("the object is written")
}

/// Whether `store` can produce `key`, asked of it in a batch of one.
async fn holds(store: &RorolalaStorage, key: &Key) -> bool {
    store
        .contains_keys(&[*key])
        .await
        .unwrap_or_default()
        .held(0)
}

/// `count` contents of about `size` bytes each, every one different from the others.
fn contents(count: usize, size: usize) -> Vec<Vec<u8>> {
    (0..count)
        .map(|at| vec![u8::try_from(at % 251).unwrap_or(0); size + at])
        .collect()
}

/// Content with repetition in it, so there is something to cut into chunks.
fn repetitive(len: usize) -> Vec<u8> {
    b"the same words over and over, "
        .iter()
        .cycle()
        .take(len)
        .copied()
        .collect()
}

/// How many loose objects the store of `name` holds under `obj/`.
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
