//! What a phase allocates, so a copy the code makes is a number rather than a reading of it.
//!
//! A sync's cost is mostly copies, and a copy in this code is an allocation and an `extend`. Counting
//! them means the global allocator, so this is one for the whole process and a snapshot is taken in
//! front of each phase; the difference between two snapshots is what that phase churned.
//!
//! What is counted is churn, not what is live: every `alloc`, `realloc` and `alloc_zeroed` adds one to
//! the count and its size to the bytes, and nothing is ever subtracted. A buffer reused a thousand
//! times is one allocation and a buffer made a thousand times is a thousand, which is exactly the
//! distinction a phase making one copy per record shows up in.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

/// How many times the process has allocated.
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

/// How many bytes the process has asked the allocator for, in total.
static BYTES: AtomicU64 = AtomicU64::new(0);

/// The allocator that counts what goes through it.
pub struct Counting;

// SAFETY: every method forwards to `System` — the same allocator the process would have used — and
// records the layout around the call. A request whose pointer comes back null is not counted, because
// nothing was allocated. `dealloc` is only ever called by the allocator on a pointer one of the other
// three methods produced.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller of `alloc` is the allocator's own contract; this forwards it unchanged.
        let pointer = unsafe { System.alloc(layout) };

        if !pointer.is_null() {
            record(layout.size());
        }

        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: as in `alloc`, forwarded unchanged.
        let pointer = unsafe { System.alloc_zeroed(layout) };

        if !pointer.is_null() {
            record(layout.size());
        }

        pointer
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: `pointer` and `layout` are the caller's contract, and `System` checks them against the
        // layout it allocated them with; this changes nothing about them.
        let moved = unsafe { System.realloc(pointer, layout, new_size) };

        if !moved.is_null() {
            record(new_size);
        }

        moved
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: `pointer` came from this allocator with this layout, which is the caller's contract.
        unsafe { System.dealloc(pointer, layout) }
    }
}

/// Records that `size` more bytes were asked for.
fn record(size: usize) {
    ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    BYTES.fetch_add(size as u64, Ordering::Relaxed);
}

/// What the counter said at one moment.
#[derive(Clone, Copy)]
pub struct Snapshot {
    /// The allocation count then.
    allocations: u64,
    /// The byte count then.
    bytes: u64,
}

/// What the allocator has counted so far.
pub fn snapshot() -> Snapshot {
    Snapshot {
        allocations: ALLOCATIONS.load(Ordering::Relaxed),
        bytes: BYTES.load(Ordering::Relaxed),
    }
}

impl Snapshot {
    /// How many allocations and bytes were made since this snapshot was taken.
    ///
    /// `realloc` counts with the new size, so what a growth costs is the whole of the new buffer rather
    /// than only the part added: a number that says how much memory a phase touched.
    pub fn since(self) -> (u64, u64) {
        let now = snapshot();

        (
            now.allocations.wrapping_sub(self.allocations),
            now.bytes.wrapping_sub(self.bytes),
        )
    }
}
