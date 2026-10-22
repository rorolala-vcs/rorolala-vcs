//! How large a split's working set was.
//!
//! A cut that copies its input, or that builds a tree of nodes over it, costs memory that the bytes
//! themselves do not show. Counting it means counting allocations, and counting allocations means the
//! global allocator — so this is one for the whole process, and the counter is reset in front of each
//! cut.
//!
//! It is deliberately crude: the high-water mark is every byte live on the heap at the highest point,
//! which includes the content the measure itself is holding. That is the number worth reporting —
//! "cutting a 4 MiB file took 9 MiB of heap" — and a figure that excluded the input would be a figure
//! nobody could compare with anything.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The bytes live on the heap now.
static LIVE: AtomicUsize = AtomicUsize::new(0);

/// The most bytes that were live at once, since the last [`reset`].
static HIGH: AtomicUsize = AtomicUsize::new(0);

/// The allocator that counts what goes through it.
pub struct Counting;

// SAFETY: every method forwards to `System` — the same allocator the process would have used — and
// does nothing with the pointer but record its layout's size around the call. `add` runs only after a
// successful allocation, so a null return is never counted, and `dealloc` is only ever called by the
// allocator on a pointer `alloc` or `realloc` produced.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller of `alloc` is the allocator's own contract; this forwards it unchanged.
        let pointer = unsafe { System.alloc(layout) };

        if !pointer.is_null() {
            add(layout.size());
        }

        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);

        // SAFETY: `pointer` came from this allocator with this layout, which is the caller's contract.
        unsafe { System.dealloc(pointer, layout) }
    }
}

/// Starts the high-water mark at whatever is live now.
///
/// The content a cut is given is already held when the cut starts, so a mark that began at zero would
/// report the cut as costing less than the file it read — and a ratio against the file's size would
/// begin below one, which nothing can mean.
pub fn reset() {
    let live = LIVE.load(Ordering::Relaxed);
    HIGH.store(live, Ordering::Relaxed);
}

/// The most bytes that were live at once, since the last [`reset`].
pub fn peak() -> usize {
    HIGH.load(Ordering::Relaxed)
}

/// Records that `size` more bytes are live.
fn add(size: usize) {
    let live = LIVE.fetch_add(size, Ordering::Relaxed).saturating_add(size);
    HIGH.fetch_max(live, Ordering::Relaxed);
}
