//! How a store decides what to do with the content it is handed.
//!
//! This is the one place a store looks at content and chooses: whether what is in front of it is worth
//! compressing, and whether it is cut and how. Nothing about the answer is written down except the
//! result of it, so the same content may be written differently on another day without any key
//! changing.
//!
//! The choice is made from the content alone — from the first bytes of it, and how long it is — so a
//! write from a file and a write from a transfer land the same way, and neither has to be told what it
//! is being given.

use std::fs;
use std::path::Path;

use super::RorolalaStorage;
use super::consts::{SNIFF_LEN, TEXT_CUT, TEXT_CUT_FROM};
use crate::{AlgorithmChoice, Chunking, Codec, ZIP_MAGIC};

impl RorolalaStorage {
    /// Chooses how `file` is to be written.
    ///
    /// This is the choice made from a file: enough of its opening is read to tell what it is, and how
    /// long it is is asked of the filesystem rather than of the content — nothing here reads the whole
    /// of anything.
    pub(super) fn choose_for_file(&self, file: &Path) -> AlgorithmChoice {
        let mut magic = [0_u8; SNIFF_LEN];
        let filled = read_magic(file, &mut magic);
        let size = fs::metadata(file).map_or(0, |data| data.len());

        self.choose_for(
            &magic[..filled],
            usize::try_from(size).unwrap_or(usize::MAX),
        )
    }

    /// Chooses how content is written, from its opening and how long it is.
    ///
    /// Three answers, and none of them needs to read the whole content to give one:
    ///
    /// - **A packed container** — anything that starts with a ZIP's signature, which is what a `.zip`
    ///   and every format built on one are — is cut at the boundaries of what it packs. See
    ///   [`Chunking::Zip`]: boundaries given rather than searched for, and the cut that can tell what
    ///   is still the same bytes it was.
    /// - **Text** is compressed, and — once there is enough of it to be worth a manifest — cut at the
    ///   ends of its lines, so that what a chunk holds is whole lines. See [`Text`]: lines are what an
    ///   edit moves, so a few of them changed costs those chunks rather than the file.
    /// - **Anything else** is written the way the store was told to write, which by default is as it
    ///   came in, whole: compressing arbitrary content is not the bargain it is for text, and cutting
    ///   it takes a manifest to say where the pieces are.
    ///
    /// What any answer comes to is not written down anywhere, so a store may be told to write
    /// differently tomorrow without a key changing.
    ///
    /// [`Text`]: crate::Chunking::Text
    pub(super) fn choose_for(&self, magic: &[u8], size: usize) -> AlgorithmChoice {
        if starts_a_container(magic) {
            return AlgorithmChoice::new(Codec::Raw, Chunking::Zip);
        }

        if is_text(magic) {
            let chunking = if size < TEXT_CUT_FROM {
                Chunking::Whole
            } else {
                TEXT_CUT
            };

            return AlgorithmChoice::new(Codec::Zstd, chunking);
        }

        AlgorithmChoice::new(self.codec, self.cut)
    }
}

/// Whether `magic` — the first bytes of some content — starts a signature this store recognises.
///
/// The only one it recognises is a ZIP's, which is what tells a packed container from content of any
/// other kind — see [`Zip`](crate::Zip) for what a container is taken apart by.
fn starts_a_container(magic: &[u8]) -> bool {
    magic.starts_with(&ZIP_MAGIC)
}

/// Whether `magic` — the first bytes of some content — reads as text.
///
/// A zero byte is what says content is not text, which is the rule `git` reads a file by: what a file
/// is *for* cannot be told from its bytes, but whether it is something a person wrote can be told well
/// enough by that much of it.
fn is_text(magic: &[u8]) -> bool {
    !magic.contains(&0)
}

/// Fills `magic` with the first bytes of `file`, and says how many of them were there.
fn read_magic(file: &Path, magic: &mut [u8]) -> usize {
    use std::io::Read as _;

    let Ok(mut handle) = fs::File::open(file) else {
        return 0;
    };

    let mut filled = 0;
    while filled < magic.len() {
        match handle.read(&mut magic[filled..]) {
            Ok(0) | Err(_) => break,
            Ok(read) => filled += read,
        }
    }

    filled
}
