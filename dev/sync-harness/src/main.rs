//! Measures what a store sync of many entries costs, one phase at a time.
//!
//! What a sync is worth is not one number: a store of small entries spends it in the asking, in what
//! the side sending reads, in what the side taking writes, and in what the channel makes of each
//! value, and only the last of those behaves like a transfer. So this program runs the real
//! [`sync_storage`] between two stores it makes, over a real encrypted session, and prints each
//! phase rather than a total.
//!
//! What it is for is choosing between shapes of the primitive — one value per entry, or one packet
//! per many; one file per entry on the receiving side, or one pack per run — and for knowing what a
//! machine charges for the parts a shape cannot change. A run says:
//!
//! ```text
//! sync-harness [--entries N] [--bytes N] [--repeat N] [--packed] [--fs] [--allocs] [--twice]
//! ```
//!
//! `--packed` packs the source before syncing, which is what a store that has been packed looks
//! like; `--fs` also measures the raw filesystem, so what the machine charges for one file written
//! and moved into place is a number beside the store's; `--allocs` also prints what each phase
//! allocated, which is where a copy shows up as a number rather than as a reading of the code;
//! `--twice` syncs the same two stores again once everything has arrived, and checks that a run with
//! nothing left to carry carries nothing and writes nothing — see [`again`].
//!
//! This is a measurement harness, so a measurement whose conditions do not hold ends with a panic: a
//! phase that did not run leaves no number, and a number taken over it would be read as what the
//! machine charges.

#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
// A measurement is reported as a fraction of a whole, and a fraction is written in floating point:
// what is measured here is microseconds and megabytes, which a mantissa of 52 bits tells apart with
// far more exactness than the clock the measurement came from.
#![allow(clippy::cast_precision_loss)]
// Four switches, and four bools is what a command line is: what a run was asked for is a set of
// independent choices, and a bitfield or an enum apiece would make the parse harder to read than the
// fields it fills.
#![allow(clippy::struct_excessive_bools)]

#[global_allocator]
static ALLOCATOR: alloc::Counting = alloc::Counting;

mod alloc;
mod pair;
mod scratch;

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use rorolala_auth::{Account, Member};
use rorolala_daemon::sync_storage;
use rorolala_protocol::{ActionContext, Both, OnlyVault, OnlyWorkspace};
use rorolala_storage::{
    Key, Lockable as _, RorolalaStorage, StorageBackend as _, TransferableBackend as _,
};

/// What a run was asked for.
struct Setting {
    /// How many entries to sync.
    entries: usize,
    /// How many bytes each entry holds.
    bytes: usize,
    /// How many times to sync them, of which the fastest is reported.
    repeat: u32,
    /// Whether to pack the source before syncing it.
    packed: bool,
    /// Whether to measure the raw filesystem as well.
    fs: bool,
    /// Whether to report what each phase allocated.
    allocs: bool,
    /// Whether to sync the same two stores again, with nothing left to carry.
    twice: bool,
    /// Whether the entries are text, which a store cuts, rather than bytes it keeps whole.
    text: bool,
}

impl Setting {
    /// The setting a command line names, and the defaults one that names none gets.
    fn parse() -> Self {
        let mut setting = Self {
            entries: 2000,
            bytes: 4096,
            repeat: 1,
            packed: false,
            fs: false,
            allocs: false,
            twice: false,
            text: false,
        };

        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--entries" => setting.entries = number(&mut args),
                "--bytes" => setting.bytes = number(&mut args),
                "--repeat" => setting.repeat = u32::try_from(number(&mut args)).unwrap_or(1),
                "--packed" => setting.packed = true,
                "--fs" => setting.fs = true,
                "--allocs" => setting.allocs = true,
                "--twice" => setting.twice = true,
                "--text" => setting.text = true,
                other => {
                    println!("sync-harness: `{other}` is not an option this build knows");
                    std::process::exit(2);
                }
            }
        }

        setting
    }
}

/// The number the next argument is, or the run says which one was not.
fn number(args: &mut impl Iterator<Item = String>) -> usize {
    args.next()
        .and_then(|written| written.parse().ok())
        .unwrap_or_else(|| {
            println!("sync-harness: an option that takes a number was given none");
            std::process::exit(2);
        })
}

#[tokio::main]
async fn main() {
    let setting = Setting::parse();
    let held = setting.entries.saturating_mul(setting.bytes);

    println!(
        "{} entries of {} bytes ({} MiB), {} run(s){}",
        setting.entries,
        setting.bytes,
        held / (1 << 20),
        setting.repeat,
        if setting.packed {
            ", source packed"
        } else {
            ""
        },
    );

    measure(&setting).await;
}

/// Fills a store with what was asked for and syncs it, saying what each phase cost.
async fn measure(setting: &Setting) {
    let source_path = scratch::at("source");
    let source = RorolalaStorage::create(&source_path);
    let phase = Phase::start();
    let keys = fill(&source, setting).await;
    phase.said("store: write the entries, one file each", setting);

    if setting.packed {
        let phase = Phase::start();
        {
            // UNWRAP: `scratch::at` emptied this directory just before the store was made, so no
            // lock file is there for `lock` to refuse.
            let guard = source.lock().await.expect("the store can be locked");

            // UNWRAP: the guard above is held, so no other holder can be writing, and the keys are
            // the ones `fill` has just laid down loose in this store.
            guard.pack(&keys).await.expect("the source packs");
        }
        phase.said("store: pack them", setting);
    }

    let phase = Phase::start();
    // UNWRAP: every key asked about is one `fill` has just written into this store, and
    // `contains_keys` answers which of them it holds rather than failing on a miss.
    let _ = source
        .contains_keys(&keys)
        .await
        .expect("the source answers");
    phase.said("store: ask which are held, all at once", setting);

    let phase = Phase::start();
    for key in &keys {
        // UNWRAP: the keys are `fill`'s own, as above; one key per call is what this phase
        // measures, not a doubt about the answer.
        let _ = source
            .contains_keys(std::slice::from_ref(key))
            .await
            .expect("the source answers");
    }
    phase.said("store: ask which are held, one key per call", setting);

    let mut last = rounds(&source, &keys, setting).await;

    let held = setting.entries.saturating_mul(setting.bytes);

    said("sync: everything, from nothing", setting, last.fastest);
    allocated(setting, last.allocs);
    println!(
        "  wire {} bytes for {} of content: {:.4} x, {:.1} bytes an entry",
        last.wire,
        held,
        last.wire as f64 / held.max(1) as f64,
        last.wire as f64 / setting.entries.max(1) as f64,
    );

    if setting.twice {
        again(setting, &mut last, (&source, &source_path), &keys).await;
    }

    if setting.fs {
        let phase = Phase::start();
        filesystem(setting);
        phase.said("filesystem: create, write and move one file", setting);
    }
}

/// The last of the timed runs: what the fastest of them cost, and the session and stores it left.
///
/// The session is kept so that the run after it can cross the one that is already there: a second
/// sync over a new session would be measuring a handshake as well, and what [`again`] is about is the
/// sync alone.
struct Last {
    /// The fastest run.
    fastest: Duration,
    /// What that run allocated.
    allocs: (u64, u64),
    /// What the last run wrote altogether, its handshake included.
    wire: u64,
    /// The Workspace's end of the session the last run used.
    workspace: ActionContext<'static>,
    /// The Vault's end of it.
    vault: ActionContext<'static>,
    /// What the Workspace wrote.
    up: pair::Counter,
    /// What the Vault wrote.
    down: pair::Counter,
    /// The store the last run filled.
    target: RorolalaStorage,
    /// Where that store is.
    target_path: PathBuf,
}

/// Syncs `source` into a fresh target the number of times the setting names.
///
/// Every run starts from an empty target, so every one of them carries the whole of `source`: what is
/// compared between runs is the machine's own variation rather than what was left over.
async fn rounds(source: &RorolalaStorage, keys: &[Key], setting: &Setting) -> Last {
    let mut fastest = Duration::MAX;
    let mut fastest_allocs = (0_u64, 0_u64);
    let mut kept = None;

    for _ in 0..setting.repeat.max(1) {
        let target_path = scratch::at("target");
        let target = RorolalaStorage::create(&target_path);
        // A few records wide: the two ends of a sync run in one task, so the buffers have to be able
        // to hold what one side writes while the other side is not being polled.
        let pair = pair::handshaken(4 << 20).await;
        let up = pair.up.clone();
        let down = pair.down.clone();

        let mut workspace =
            ActionContext::new_workspace_ctx(Account::default()).with_channel(pair.workspace);
        let mut vault = ActionContext::new_vault_ctx(Member::default()).with_channel(pair.vault);

        let here = source.clone();
        let there = target.clone();
        let workspace_local = OnlyWorkspace::new(&workspace, || here);
        // PANIC: `vault` is a Vault context, so `OnlyWorkspace::new` takes its `empty()` branch and
        // the closure is never called.
        let vault_other = OnlyWorkspace::new(&vault, || unreachable!("the Vault holds nothing"));
        // PANIC: `workspace` is a Workspace context, so `OnlyVault::new` takes its `empty()` branch
        // and the closure is never called.
        let workspace_other = OnlyVault::new(&workspace, || unreachable!("nothing to take yet"));
        let vault_local = OnlyVault::new(&vault, || there);

        let asked = keys.to_vec();
        let also_asked = keys.to_vec();

        let phase = Phase::start();
        let (from_workspace, from_vault) = tokio::join!(
            sync_storage(
                &mut workspace,
                workspace_local,
                workspace_other,
                Both::new(asked),
            ),
            sync_storage(&mut vault, vault_other, vault_local, Both::new(also_asked)),
        );
        let elapsed = phase.started.elapsed();
        let allocs = phase.allocs.since();

        // UNWRAP: the sync ran between two stores this run made over a channel `pair::handshaken`
        // has completed, so a refusal is the primitive failing and leaves no phase to report.
        from_workspace.expect("the Workspace side of the sync succeeds");
        // UNWRAP: the same pair, the other end: the channel it reads was handshaken before either
        // side was asked, so an error here is the Vault half of the primitive failing.
        from_vault.expect("the Vault side of the sync succeeds");

        // UNWRAP: the target is the store `RorolalaStorage::create` has just made, so it has an
        // index to ask about the keys the sync has laid into it.
        let arrived = target
            .contains_keys(keys)
            .await
            .expect("the target answers");
        // PANIC: the message is this measurement's own invariant, and a target short of what crossed
        // is a wrong number rather than a slower one, so the run stops instead of reporting it.
        assert!(
            (0..keys.len()).all(|at| arrived.held(at)),
            "the target holds everything that crossed",
        );

        if elapsed < fastest {
            fastest = elapsed;
            fastest_allocs = allocs;
        }
        kept = Some(Last {
            fastest,
            allocs: fastest_allocs,
            wire: up.load(Ordering::Relaxed) + down.load(Ordering::Relaxed),
            workspace,
            vault,
            up,
            down,
            target,
            target_path,
        });
    }

    // PANIC: a loop that runs at least once always leaves one, and `repeat` is floored to one.
    kept.expect("a run syncs at least once")
}

/// Syncs the same two stores again, over the session that carried the first sync.
///
/// This is the idempotency check. The first sync left both sides holding everything, so the second
/// one has nothing to carry and nothing to write — and what it does cost is a number this can name:
/// the two presence answers, which are one byte per key each, and the value that says the run
/// reached the other end. A byte more is a run that moved something it should not have.
///
/// The stores are compared by what they hold rather than by when they were touched: what a store is,
/// is the files under it and the bytes in them.
async fn again(
    setting: &Setting,
    last: &mut Last,
    from: (&RorolalaStorage, &PathBuf),
    keys: &[Key],
) {
    let sent = tree(&last.target_path);
    let here = tree(from.1);

    let before = last.up.load(Ordering::Relaxed) + last.down.load(Ordering::Relaxed);
    let phase = Phase::start();

    let workspace_local = OnlyWorkspace::new(&last.workspace, || from.0.clone());
    // PANIC: the Vault's context is a Vault context, so `OnlyWorkspace::new` takes its `empty()`
    // branch and the closure is never called.
    let vault_other = OnlyWorkspace::new(&last.vault, || unreachable!("the Vault holds nothing"));
    // PANIC: the Workspace's context is a Workspace context, so `OnlyVault::new` takes its `empty()`
    // branch and the closure is never called.
    let workspace_other = OnlyVault::new(&last.workspace, || unreachable!("nothing to take yet"));
    let vault_local = OnlyVault::new(&last.vault, || last.target.clone());

    let (from_workspace, from_vault) = tokio::join!(
        sync_storage(
            &mut last.workspace,
            workspace_local,
            workspace_other,
            Both::new(keys.to_vec()),
        ),
        sync_storage(
            &mut last.vault,
            vault_other,
            vault_local,
            Both::new(keys.to_vec()),
        ),
    );

    let elapsed = phase.started.elapsed();
    let allocs = phase.allocs.since();

    // UNWRAP: the first sync over this session succeeded, and the stores it left hold everything, so
    // a refusal here is the second run finding something the first one said it had already done.
    from_workspace.expect("the Workspace side of the second sync succeeds");
    // UNWRAP: the same session, the other end.
    from_vault.expect("the Vault side of the second sync succeeds");

    let moved = last.up.load(Ordering::Relaxed) + last.down.load(Ordering::Relaxed) - before;
    let expected = again_bytes(keys.len());

    println!(
        "{:<44} {:>9.2?}",
        "sync: again, with nothing left to carry", elapsed
    );
    allocated(setting, allocs);
    println!(
        "  wire {moved} bytes, {expected} expected for {} keys",
        keys.len()
    );

    // PANIC: a run that carried anything the second time is not a slower run but a wrong one, so it
    // stops rather than reporting a number a reader would compare with the first.
    assert_eq!(
        moved, expected,
        "a sync with nothing left to carry carried something"
    );
    assert_eq!(
        tree(&last.target_path),
        sent,
        "the second sync left the target as it was"
    );
    assert_eq!(tree(from.1), here, "the second sync wrote to the source");
    println!("  both stores unchanged: {}", sent.to_hex());
}

/// What a sync with nothing to carry costs, in bytes.
///
/// The two presence answers, then the value that says the run reached the other end. An answer is one
/// byte per key, and every value is a frame's length in front of it and a record's length and tag
/// around it, whether it is a presence answer or the eight-byte value that ends the run.
const fn again_bytes(keys: usize) -> u64 {
    let answer = keys as u64 + 4 + 20;

    2 * answer + 8 + 4 + 20
}

/// Every file under `root`, by path and by content, as one digest.
fn tree(root: &std::path::Path) -> blake3::Hash {
    let mut hasher = blake3::Hasher::new();
    let mut files = Vec::new();
    walk(root, root, &mut files);
    files.sort();

    for path in files {
        hasher.update(path.to_string_lossy().as_bytes());

        // UNWRAP: the walk collects the regular files it has just seen, and nothing else is writing
        // to either store while this runs, so a file that cannot be read is a check that cannot be
        // taken rather than a store with an answer.
        let bytes = std::fs::read(root.join(&path)).expect("a store's file reads back");
        hasher.update(&(bytes.len() as u64).to_be_bytes());
        hasher.update(&bytes);
    }

    hasher.finalize()
}

/// Collects every regular file under `at`, as a path relative to `root`.
fn walk(root: &std::path::Path, at: &std::path::Path, files: &mut Vec<PathBuf>) {
    // UNWRAP: both directories belong to stores this run made, so one that cannot be read is a check
    // that cannot be taken.
    for entry in std::fs::read_dir(at).expect("a store's directory reads") {
        let path = entry.expect("a directory entry reads").path();

        if path.is_dir() {
            walk(root, &path, files);
        } else {
            files.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
        }
    }
}

/// A phase being measured: when it started and what the allocator had counted then.
struct Phase {
    /// When the phase started.
    started: Instant,
    /// What the allocator had counted when it started.
    allocs: alloc::Snapshot,
}

impl Phase {
    /// Starts measuring.
    fn start() -> Self {
        Self {
            started: Instant::now(),
            allocs: alloc::snapshot(),
        }
    }

    /// Says what the phase cost, and what it allocated when the run was asked to count.
    fn said(&self, label: &str, setting: &Setting) {
        let cost = self.started.elapsed();
        // Before printing: a report that allocated would otherwise charge its own formatting to the
        // phase it is reporting.
        let allocs = self.allocs.since();

        said(label, setting, cost);
        allocated(setting, allocs);
    }
}

/// Says how many allocations a phase made, if the run was asked to count them.
fn allocated(setting: &Setting, (allocations, bytes): (u64, u64)) {
    if setting.allocs {
        let megabytes = bytes as f64 / (1024.0 * 1024.0);

        println!("  {allocations} allocations, {megabytes:.2} MiB asked for");
    }
}

/// Says what one phase cost, and what a single entry of it came to.
fn said(label: &str, setting: &Setting, cost: Duration) {
    let each = cost.as_secs_f64() / setting.entries.max(1) as f64;
    let megabytes = (setting.entries.saturating_mul(setting.bytes)) as f64 / 1e6;

    println!(
        "{label:<44} {:>9.2?}  {:>8.1} µs/entry  {:>7.2} MB/s",
        cost,
        each * 1_000_000.0,
        megabytes / cost.as_secs_f64().max(1e-9),
    );
}

/// What the machine charges for writing one file and moving it into place, which is what laying an
/// entry down one file each comes to before any of the store's own work.
fn filesystem(setting: &Setting) {
    let directory = scratch::at("filesystem");
    let content = vec![0_u8; setting.bytes];

    for at in 0..setting.entries {
        let path = directory.join(format!("e{at:07}"));
        let temporary = PathBuf::from(format!("{}.tmp", path.display()));

        let _ = std::fs::write(&temporary, &content);
        let _ = std::fs::rename(&temporary, &path);
    }

    let _ = std::fs::remove_dir_all(&directory);
}

/// Writes the entries the setting names into `store` and answers their keys.
///
/// What is written does not repeat: entries that deduplicated against each other would measure a
/// store that writes less than it was asked to.
async fn fill(store: &RorolalaStorage, setting: &Setting) -> Vec<Key> {
    let mut keys = Vec::with_capacity(setting.entries);

    for at in 0..setting.entries {
        let content = body(at, setting);
        let key = Key::new(*blake3::hash(&content).as_bytes());

        // UNWRAP: the key is the `blake3` hash of the very bytes it goes with, so the store has a
        // verified entry to lay into this run's own scratch directory.
        store.accept(&key, content).await.expect("the store writes");
        keys.push(key);
    }

    keys
}

/// Deterministic content that does not repeat, so every entry is one entry.
///
/// Text is what a store cuts, so an entry of text arrives as a manifest and the chunks it names —
/// which is the case a sync of many small entries is really about — while anything else is kept
/// whole and crosses as one entry of its own.
fn body(at: usize, setting: &Setting) -> Vec<u8> {
    let mut content = vec![0_u8; setting.bytes];
    let mut state = (at as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(0x1234_5678);

    for byte in &mut content {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *byte = (state & 0xFF) as u8;
    }

    if setting.text {
        // Lines of letters, which is what the store's own rule reads as text: no zero byte, and a
        // line's end every so often so that the cut has something to cut at.
        for (at, byte) in content.iter_mut().enumerate() {
            let letter = u8::try_from(at % 26).unwrap_or(0);

            *byte = if at % 64 == 63 {
                b'\n'
            } else {
                b'a'.saturating_add(letter)
            };
        }
    }

    content
}
