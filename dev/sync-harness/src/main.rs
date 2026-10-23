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
//! sync-harness [--entries N] [--bytes N] [--repeat N] [--packed] [--fs]
//! ```
//!
//! `--packed` packs the source before syncing, which is what a store that has been packed looks
//! like; `--fs` also measures the raw filesystem, so what the machine charges for one file written
//! and moved into place is a number beside the store's.

#![deny(warnings)]
#![deny(rust_2018_idioms)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
// A measurement is reported as a fraction of a whole, and a fraction is written in floating point:
// what is measured here is microseconds and megabytes, which a mantissa of 52 bits tells apart with
// far more exactness than the clock the measurement came from.
#![allow(clippy::cast_precision_loss)]

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
    let source = RorolalaStorage::create(scratch::at("source"));
    let started = Instant::now();
    let keys = fill(&source, setting).await;
    said(
        "store: write the entries, one file each",
        setting,
        started.elapsed(),
    );

    if setting.packed {
        let started = Instant::now();
        {
            let guard = source.lock().await.expect("the store can be locked");

            guard.pack(&keys).await.expect("the source packs");
        }
        said("store: pack them", setting, started.elapsed());
    }

    let started = Instant::now();
    let _ = source
        .contains_keys(&keys)
        .await
        .expect("the source answers");
    said(
        "store: ask which are held, all at once",
        setting,
        started.elapsed(),
    );

    let started = Instant::now();
    for key in &keys {
        let _ = source
            .contains_keys(std::slice::from_ref(key))
            .await
            .expect("the source answers");
    }
    said(
        "store: ask which are held, one key per call",
        setting,
        started.elapsed(),
    );

    let mut fastest = Duration::MAX;
    let mut wire = 0_u64;

    for _ in 0..setting.repeat.max(1) {
        let target = RorolalaStorage::create(scratch::at("target"));
        let pair = pair::handshaken(1 << 20).await;
        let up = pair.up.clone();
        let down = pair.down.clone();

        let mut workspace =
            ActionContext::new_workspace_ctx(Account::default()).with_channel(pair.workspace);
        let mut vault = ActionContext::new_vault_ctx(Member::default()).with_channel(pair.vault);

        let here = source.clone();
        let there = target.clone();
        let workspace_local = OnlyWorkspace::new(&workspace, || here);
        let vault_other = OnlyWorkspace::new(&vault, || unreachable!("the Vault holds nothing"));
        let workspace_other = OnlyVault::new(&workspace, || unreachable!("nothing to take yet"));
        let vault_local = OnlyVault::new(&vault, || there);

        let asked = keys.clone();
        let also_asked = keys.clone();

        let started = Instant::now();
        let (from_workspace, from_vault) = tokio::join!(
            sync_storage(
                &mut workspace,
                workspace_local,
                workspace_other,
                Both::new(asked),
            ),
            sync_storage(&mut vault, vault_other, vault_local, Both::new(also_asked)),
        );
        let elapsed = started.elapsed();

        from_workspace.expect("the Workspace side of the sync succeeds");
        from_vault.expect("the Vault side of the sync succeeds");

        let arrived = target
            .contains_keys(&keys)
            .await
            .expect("the target answers");
        assert!(
            (0..keys.len()).all(|at| arrived.held(at)),
            "the target holds everything that crossed",
        );

        fastest = fastest.min(elapsed);
        wire = up.load(Ordering::Relaxed) + down.load(Ordering::Relaxed);
    }

    let held = setting.entries.saturating_mul(setting.bytes);

    said("sync: everything, from nothing", setting, fastest);
    println!(
        "  wire {} bytes for {} of content: {:.4} x, {:.1} bytes an entry",
        wire,
        held,
        wire as f64 / held.max(1) as f64,
        wire as f64 / setting.entries.max(1) as f64,
    );

    if setting.fs {
        said(
            "filesystem: create, write and move one file",
            setting,
            filesystem(setting),
        );
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
fn filesystem(setting: &Setting) -> Duration {
    let directory = scratch::at("filesystem");
    let content = vec![0_u8; setting.bytes];
    let started = Instant::now();

    for at in 0..setting.entries {
        let path = directory.join(format!("e{at:07}"));
        let temporary = PathBuf::from(format!("{}.tmp", path.display()));

        let _ = std::fs::write(&temporary, &content);
        let _ = std::fs::rename(&temporary, &path);
    }

    let _ = std::fs::remove_dir_all(&directory);

    started.elapsed()
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
