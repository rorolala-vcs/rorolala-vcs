//! Variant files the tree holds somewhere other than where the merge records put them.
//!
//! A variant file lies where a merge record reads it from the target's path, and a run that moves it
//! with the filesystem rather than with `rola fs-ops mv` leaves the record saying one thing and the
//! tree holding another. The file is found again by what it holds: a variant names the content it
//! was brought in with, and a move does not change that, so the untagged file whose digest is the
//! variant's own is the one that moved.
//!
//! Finding it here rather than in the reading of the tree is deliberate: a variant file is not a
//! path the Layout names, so it has no place in the reading, and what it is to be compared against
//! is the index rather than the Layout. A move found this way is the same kind of thing a Layout
//! path's move is: something for `rola align` to confirm.

use std::path::Path;

use rorolala_layout::{Layout, LayoutPath};
use rorolala_storage::{Blake3Hash, Key};
use rorolala_vcs::{VCSIndex, VCSIndexObject};
use rorolala_workspace::Merging;
use rorolala_workspace_analyze::entry_of;
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
pub fn variant_moves(
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

/// The content the variant `hash` was brought in with, when the index holds it.
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use rorolala_layout::{Layout, LayoutPath, MutableData};
    use rorolala_vcs::{VCSIndex, Variant};
    use rorolala_workspace::{Merging, Pending};
    use rorolala_workspace_analyze::entry_of;
    use uuid::Uuid;

    use super::storage_of;
    use super::variant_moves;

    /// A workspace-like directory with a Layout that names one file.
    struct Held {
        dir: PathBuf,
        layout: Layout,
        id: Uuid,
        index: VCSIndex,
        runtime: tokio::runtime::Runtime,
    }

    fn scratch(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rola-status-merge-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        dir
    }

    /// A Layout naming `art.psd` by `id`, with an index beside it.
    fn held(label: &str) -> Held {
        let dir = scratch(label);
        let layout = Layout::open(&dir).unwrap();
        let id = Uuid::from_u128(1);

        layout
            .create_entry(id, MutableData::new(None, [1_u8; 32], "art".to_owned()))
            .unwrap();
        layout
            .create_path(&LayoutPath::new("art.psd").unwrap(), id)
            .unwrap();

        Held {
            index: VCSIndex::create(dir.join(".rola/index")),
            runtime: tokio::runtime::Runtime::new().unwrap(),
            dir,
            layout,
            id,
        }
    }

    #[test]
    fn a_reading_with_no_merge_in_progress_finds_no_move() {
        let dir = scratch("none");
        let layout = Layout::open(&dir).unwrap();
        let index = VCSIndex::create(dir.clone());
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let merging = Merging::default();

        assert!(variant_moves(&merging, &layout, &dir, None, &runtime, &[]).is_empty());
        assert!(variant_moves(&merging, &layout, &dir, Some(&index), &runtime, &[]).is_empty());

        let missing = LayoutPath::from_relative("missing.psd".as_ref()).unwrap();
        assert!(
            variant_moves(&merging, &layout, &dir, Some(&index), &runtime, &[missing]).is_empty()
        );

        assert!(storage_of(&index, &runtime, &[0; 32]).is_none());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_variant_file_found_by_its_content_is_reported_as_a_move() {
        let held = held("moved");
        let content = held.dir.join("elsewhere.psd");
        std::fs::write(&content, b"variant content").unwrap();

        // The variant was brought in with the content that now lies elsewhere, so the digest the
        // reading finds is the one the index holds for it.
        let digest = entry_of(&content).unwrap().digest();
        let variant =
            Variant::new_bare_variant(digest, [0_u8; 32], None, [0_u8; 32], [0_u8; 32], 0);
        let key = held.runtime.block_on(held.index.write(variant)).unwrap();

        let mut merging = Merging::default();
        merging.push(Pending::new(
            held.id,
            *key.digest(),
            Some(PathBuf::from("gone.psd")),
        ));
        let untagged = [LayoutPath::new("elsewhere.psd").unwrap()];

        let moves = variant_moves(
            &merging,
            &held.layout,
            &held.dir,
            Some(&held.index),
            &held.runtime,
            &untagged,
        );

        assert_eq!(moves.len(), 1);
        assert_eq!(moves[0].target, held.id);
        assert_eq!(moves[0].variant, *key.digest());
        assert_eq!(moves[0].from.as_str(), "gone.psd");
        assert_eq!(moves[0].to.as_str(), "elsewhere.psd");

        let _ = std::fs::remove_dir_all(held.dir);
    }

    #[test]
    fn a_record_that_cannot_be_matched_is_left_alone() {
        let held = held("unmatched");

        let mut merging = Merging::default();
        // A target the Layout does not name.
        merging.push(Pending::new(
            Uuid::from_u128(2),
            [4_u8; 32],
            Some(PathBuf::from("gone.psd")),
        ));
        // A variant the index does not hold.
        merging.push(Pending::new(
            held.id,
            [5_u8; 32],
            Some(PathBuf::from("gone.psd")),
        ));
        // A variant the index holds whose content is not among the untagged paths.
        let variant =
            Variant::new_bare_variant([6_u8; 32], [0_u8; 32], None, [0_u8; 32], [0_u8; 32], 0);
        let key = held.runtime.block_on(held.index.write(variant)).unwrap();
        merging.push(Pending::new(
            held.id,
            *key.digest(),
            Some(PathBuf::from("gone.psd")),
        ));

        let moves = variant_moves(
            &merging,
            &held.layout,
            &held.dir,
            Some(&held.index),
            &held.runtime,
            &[],
        );

        assert!(moves.is_empty());

        let _ = std::fs::remove_dir_all(held.dir);
    }
}
