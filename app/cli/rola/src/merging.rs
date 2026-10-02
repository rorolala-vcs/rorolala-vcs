//! Variant files the tree holds somewhere other than where the merge records put them.
//!
//! A variant file lies where a merge record reads it from the target's path, and a run that moves it
//! with the filesystem rather than with `rola fs-ops mv` leaves the record saying one thing and the
//! tree holding another. The file is found again by what it holds: a variant names the content it
//! was checked in with, and a move does not change that, so the untagged file whose digest is the
//! variant's own is the one that moved.
//!
//! Finding it here rather than in the reading of the tree is deliberate: a variant file is not a
//! path the Layout names, so it has no place in the reading, and what it is to be compared against
//! is the index rather than the Layout. A move found this way is the same kind of thing a Layout
//! path's move is: something for `rola align` to confirm.

use std::path::Path;

use librorolala::layout::{Layout, LayoutPath};
use librorolala::storage::{Blake3Hash, Key};
use librorolala::tree_analyze::entry_of;
use librorolala::vcs::{VCSIndex, VCSIndexObject};
use librorolala::workspace::Merging;
use uuid::Uuid;

/// A variant file the tree holds somewhere other than where the merge record puts it.
pub struct VariantMove {
    /// The file the variant is to be joined into.
    pub target: Uuid,
    /// The variant whose file moved.
    pub variant: Blake3Hash,
    /// Where the record puts the file.
    pub from: LayoutPath,
    /// Where the tree holds it.
    pub to: LayoutPath,
}

/// The variant files the tree holds somewhere other than where the merge records put them.
///
/// A record whose file is where it says is nothing to find, and one whose content is not in the
/// index is nothing to match by. What is left is looked for among the paths the Layout does not
/// name, by the content each holds; a path is claimed by the first record that names it, so two
/// records cannot be answered with one file.
#[must_use]
pub fn moves(
    merging: &Merging,
    layout: &Layout,
    root: &Path,
    index: Option<&VCSIndex>,
    runtime: &tokio::runtime::Runtime,
    untagged: &[LayoutPath],
) -> Vec<VariantMove> {
    let Some(index) = index else {
        return Vec::new();
    };

    // What each record expects, for the records whose file is not where they say. A run with nothing
    // missing reads no path at all, which is the run that pays nothing for this.
    let missing: Vec<_> = merging
        .iter()
        .filter(|pending| {
            merging
                .variant_path(layout, pending)
                .is_some_and(|path| !root.join(path.to_path_buf()).is_file())
        })
        .collect();

    if missing.is_empty() {
        return Vec::new();
    }

    let mut found: Vec<(Blake3Hash, LayoutPath)> = Vec::new();
    for path in untagged {
        if let Ok(entry) = entry_of(&root.join(path.to_path_buf())) {
            found.push((entry.digest(), path.clone()));
        }
    }

    let mut claimed = Vec::new();
    let mut moves = Vec::new();

    for pending in missing {
        let Some(from) = merging.variant_path(layout, pending) else {
            continue;
        };
        let Some(stored) = storage_of(index, runtime, pending.variant()) else {
            continue;
        };
        let Some((_, to)) = found
            .iter()
            .find(|(digest, path)| *digest == stored && !claimed.contains(path))
            .cloned()
        else {
            continue;
        };

        claimed.push(to.clone());
        moves.push(VariantMove {
            target: pending.target(),
            variant: *pending.variant(),
            from,
            to,
        });
    }

    moves
}

/// The content the variant `hash` was checked in with, when the index holds it.
fn storage_of(
    index: &VCSIndex,
    runtime: &tokio::runtime::Runtime,
    hash: &Blake3Hash,
) -> Option<Blake3Hash> {
    match runtime.block_on(index.read(Key::new(*hash))) {
        Ok(VCSIndexObject::Variant(variant)) => Some(*variant.storage_hash()),
        _ => None,
    }
}
