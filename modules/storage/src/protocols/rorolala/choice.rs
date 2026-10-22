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
    ///
    /// The opening is read with a blocking `std` handle rather than through `tokio::fs`, which is what
    /// [`choose_algorithm`](crate::StorageBackend::choose_algorithm) is synchronous for: one read of
    /// at most [`SNIFF_LEN`] bytes, in front of a write that reads the whole file. A caller on a task
    /// blocks it for about as long as that read takes, which is the same order as the metadata
    /// lookup beside it.
    pub(super) fn choose_for_file(&self, file: &Path) -> AlgorithmChoice {
        let mut magic = [0_u8; SNIFF_LEN];
        let filled = read_magic(file, &mut magic);
        let size = fs::metadata(file).map_or(0, |data| data.len());

        // A container is claimed by its extension, and an extension is written by whoever saved the
        // file: `.MB` and `.mb` are one container to them, so they are one here. Lowercasing is one
        // small allocation in front of a write that reads the whole file.
        let extension = file
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase);

        self.choose_for(
            &magic[..filled],
            usize::try_from(size).unwrap_or(usize::MAX),
            extension.as_deref(),
        )
    }

    /// Chooses how content is written, from a cut it was told, a container it claims, its opening and
    /// how long it is.
    ///
    /// Four answers, in this order, and none of them needs to read the whole content to give one:
    ///
    /// - **A cut that was asked for** wins over every other answer: a store told which strategy a
    ///   write is to use is not being advised, so neither a claim nor a signature may overrule it.
    /// - **A container a policy claims** is cut by the policy that claims it. The claim is the one
    ///   thing here that knows what the bytes are without reading them, which is what makes it the
    ///   cheapest of the answers and the most predictable.
    /// - **A packed container** — anything that starts with a ZIP's signature, which is what a `.zip`
    ///   and every format built on one are — is cut at the boundaries of what it packs. See
    ///   [`Chunking::Zip`]: boundaries given rather than searched for, and the cut that can tell what
    ///   is still the same bytes it was.
    /// - **Text** is cut at the ends of its lines once there is enough of it to be worth a manifest,
    ///   so that what a chunk holds is whole lines. See [`Text`]: lines are what an edit moves, so a
    ///   few of them changed costs those chunks rather than the file.
    /// - **Anything else** is written as it came in, whole: cutting it takes a manifest to say where
    ///   the pieces are, and that is a cost nothing pays for itself. That is also what this store did
    ///   before there were claims at all, so a store with no policy and nothing forced writes exactly
    ///   what it used to.
    ///
    /// What any answer comes to is not written down anywhere, so a store may be told to write
    /// differently tomorrow without a key changing.
    ///
    /// [`Text`]: crate::Chunking::Text
    pub(super) fn choose_for(
        &self,
        magic: &[u8],
        size: usize,
        extension: Option<&str>,
    ) -> AlgorithmChoice {
        // The encoding and the cut are two decisions, taken apart from each other. What a caller
        // forces is a cut — a policy names how content is cut, not how it is stored — so forcing one
        // leaves the other where the content put it: text stays compressed and a container stays as
        // it came in.
        let container = starts_a_container(magic);
        let codec = if container {
            Codec::Raw
        } else if is_text(magic) {
            Codec::Zstd
        } else {
            self.codec
        };

        AlgorithmChoice::new(codec, self.choose_cut(magic, size, extension, container))
    }

    /// Which cut, in the order this store decides it.
    fn choose_cut(
        &self,
        magic: &[u8],
        size: usize,
        extension: Option<&str>,
        container: bool,
    ) -> Chunking {
        if let Some(cut) = self.cut {
            return cut;
        }

        if let Some(extension) = extension
            && let Some(claimed) = crate::claiming(extension)
        {
            return Chunking::Policy(claimed.name);
        }

        if container {
            return Chunking::Zip;
        }

        if is_text(magic) && size >= TEXT_CUT_FROM {
            return TEXT_CUT;
        }

        Chunking::Whole
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
