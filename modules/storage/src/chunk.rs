use std::ops::Range;

/// How content is cut into chunks before it is written.
///
/// Cutting is a **write-time** choice and nothing else. What a chunked entry records is the
/// ordered list of the chunks it was made of — see [`Manifest`](crate::Manifest) — which says
/// nothing about how the cuts were found. So content cut by *any* chunker reads back the same:
/// a fixed-size one, a content-defined one, or one that does not exist yet. That is what makes
/// the boundary algorithm free to change, and free to be chosen later.
///
/// A chunker must be **deterministic**: the same content cuts the same way every time. Anything
/// else would give the same file two different manifests on two writes, and the store would hold
/// the same content twice under one key.
///
/// Chunking is not remembered anywhere: a reader reassembles from the manifest alone, so a
/// chunker that was used to write yesterday's objects does not have to exist to read them.
pub trait Chunker {
    /// The ranges of `content` that are stored as chunks of their own.
    ///
    /// The ranges are in order, do not overlap, and cover all of `content`, so concatenating the
    /// pieces they name gives `content` back exactly. Content that is empty is covered by
    /// nothing, so an empty content may cut into no chunks at all — which a store lays down as an
    /// empty object, exactly as it lays down any content that is not cut.
    fn split(&self, content: &[u8]) -> Vec<Range<usize>>;
}

/// A chunker that never cuts: the whole content is one chunk.
///
/// One chunk is what an entry that was not cut is made of, and a store lays that down as a single
/// object rather than a manifest — so this is also the answer to "cut nothing".
#[derive(Debug, Clone, Copy, Default)]
pub struct Whole;

impl Chunker for Whole {
    fn split(&self, content: &[u8]) -> Vec<Range<usize>> {
        std::iter::once(0..content.len()).collect()
    }
}

/// A chunker that cuts every `size` bytes.
///
/// Cutting by position is the cheapest cut there is, and it is often enough for content that is
/// written once and read whole. It is also the cut that ages worst: inserting a byte in the
/// middle of a file shifts everything after it, so every later chunk is a chunk that was not
/// there before — see [`Cdc`] for the cut that does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fixed {
    /// How many bytes each chunk holds, bar the last.
    size: u32,
}

impl Fixed {
    /// A chunker cutting every `size` bytes.
    #[must_use]
    pub const fn new(size: u32) -> Self {
        Self { size }
    }

    /// How many bytes each chunk holds, bar the last.
    #[must_use]
    pub const fn size(&self) -> u32 {
        self.size
    }
}

impl Chunker for Fixed {
    fn split(&self, content: &[u8]) -> Vec<Range<usize>> {
        // A size of nothing would cut forever without getting anywhere, so it is read as one.
        let size = (self.size as usize).max(1);
        let len = content.len();

        (0..len)
            .step_by(size)
            .map(|start| start..(start + size).min(len))
            .collect()
    }
}

/// A chunker that cuts where the content says so.
///
/// The cut points come from a rolling hash of the bytes rather than from how far along they are,
/// so inserting or removing something in the middle of a file only disturbs the chunk it lands
/// in: the chunks around it are found in the same places they were before. That is what makes
/// editing a large file cheap to store again, and why a content-defined cut is worth its cost
/// where a [`Fixed`] one is not.
///
/// `average` is the size a chunk tends towards; `min` and `max` bound how small and how large a
/// chunk may get. None of them is written down anywhere — a reader reassembles from the
/// manifest — so they may be tuned, or the whole algorithm replaced, without a store having to be
/// rewritten.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cdc {
    /// The smallest a chunk may be.
    min: u32,
    /// The size a chunk tends towards.
    average: u32,
    /// The largest a chunk may be.
    max: u32,
}

impl Cdc {
    /// A chunker cutting on content, tending towards `average` and staying within `min` and
    /// `max`.
    #[must_use]
    pub const fn new(min: u32, average: u32, max: u32) -> Self {
        Self { min, average, max }
    }

    /// The smallest a chunk may be.
    #[must_use]
    pub const fn min(&self) -> u32 {
        self.min
    }

    /// The size a chunk tends towards.
    #[must_use]
    pub const fn average(&self) -> u32 {
        self.average
    }

    /// The largest a chunk may be.
    #[must_use]
    pub const fn max(&self) -> u32 {
        self.max
    }
}

impl Chunker for Cdc {
    fn split(&self, content: &[u8]) -> Vec<Range<usize>> {
        let len = content.len();

        // A bound of nothing would read as "never cut" or "never stop", so the bounds are held to
        // something a cut can actually respect: no more than `max` bytes a chunk, and progress
        // that always ends.
        let max = (self.max as usize).max(1);
        let min = (self.min as usize).min(max);
        let mask = mask_for(self.average);

        let mut cuts = Vec::new();
        let mut start = 0;

        while start < len {
            let limit = start.saturating_add(max).min(len);
            let mut hash = 0_u64;
            let mut end = limit;
            let mut position = start;

            while position < limit {
                hash = (hash << 1).wrapping_add(gear(content[position]));
                position += 1;

                if position - start >= min && hash & mask == 0 {
                    end = position;
                    break;
                }
            }

            cuts.push(start..end);
            start = end;
        }

        cuts
    }
}

/// A chunker that cuts text at the ends of its lines.
///
/// The cuts are found the way [`Cdc`] finds them — a rolling hash over the bytes — but each one is
/// moved on to the next end of line, so what a chunk holds is whole lines. That is what makes a
/// chunk the thing an edit actually moves: lines are added, changed and dropped whole, so the lines
/// either side of an edit are the same lines, and therefore the same bytes, and therefore the same
/// chunk — in this version of the file, in another one, or in another file altogether.
///
/// Text is also where a small cut pays for itself: a few lines changed in a large file costs the
/// chunks those lines fall in rather than the file, which is why the sizes below are small.
///
/// `min`, `average` and `max` bound the cuts the way [`Cdc`]'s do, with one allowance: a cut is
/// moved on to the end of the line it lands in, so a chunk may come out up to one line longer than
/// `max` where that line would otherwise be cut in two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Text {
    /// The smallest a chunk may be.
    min: u32,
    /// The size a chunk tends towards.
    average: u32,
    /// The largest a chunk may be.
    max: u32,
}

impl Text {
    /// A chunker cutting at line ends, tending towards `average` and staying within `min` and
    /// `max`.
    #[must_use]
    pub const fn new(min: u32, average: u32, max: u32) -> Self {
        Self { min, average, max }
    }

    /// The smallest a chunk may be.
    #[must_use]
    pub const fn min(&self) -> u32 {
        self.min
    }

    /// The size a chunk tends towards.
    #[must_use]
    pub const fn average(&self) -> u32 {
        self.average
    }

    /// The largest a chunk may be.
    #[must_use]
    pub const fn max(&self) -> u32 {
        self.max
    }
}

impl Chunker for Text {
    fn split(&self, content: &[u8]) -> Vec<Range<usize>> {
        // The cuts are found as content of this kind would find them, and then each is moved on to
        // the end of the line it landed in: a chunk that began or ended in the middle of a line
        // would be a chunk that the same lines could not share.
        let cuts = Cdc::new(self.min, self.average, self.max).split(content);
        let mut ranges = Vec::with_capacity(cuts.len());
        let mut at = 0;

        for cut in cuts.iter().take(cuts.len().saturating_sub(1)) {
            let end = line_end_after(content, cut.end);

            if end > at {
                ranges.push(at..end);
                at = end;
            }
        }

        if at < content.len() {
            ranges.push(at..content.len());
        }

        ranges
    }
}

/// Where the first line that ends at or after `from` ends.
///
/// A line ends after its newline, and a stretch with no newline in it is one line to the end of the
/// content.
fn line_end_after(content: &[u8], from: usize) -> usize {
    let Some(rest) = content.get(from..) else {
        return content.len();
    };

    rest.iter()
        .position(|byte| *byte == b'\n')
        .map_or(content.len(), |at| from + at + 1)
}

/// The mask that makes a cut land about every `average` bytes.
///
/// A rolling hash is as likely to end in any bit pattern, so cutting wherever the low bits are
/// zero cuts once every `2^n` bytes — which is the power of two nearest `average`.
fn mask_for(average: u32) -> u64 {
    u64::from(average).max(1).next_power_of_two() - 1
}

/// The value a byte contributes to the rolling hash.
///
/// It is fixed per byte value, so the same bytes always cut in the same places: a chunker that
/// answered differently on another day would give one file two different manifests. The value is
/// a `splitmix64` round of the byte, which is a table of random-looking numbers without having to
/// be spelled out as one.
fn gear(byte: u8) -> u64 {
    let mut mixed = u64::from(byte).wrapping_add(0x9E37_79B9_7F4A_7C15);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);

    mixed ^ (mixed >> 31)
}

/// How content is to be cut into chunks before it is written.
///
/// This is the choice, where [`Chunker`] is the doing: it says *which* cut a write will use, and
/// it is what a backend's algorithm choice carries. Since the boundary algorithm is never
/// recorded — a reader reassembles from the manifest alone — adding one is adding a variant here
/// and nothing else, and nothing already written changes.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Chunking {
    /// The content is not cut: it is stored as one chunk.
    #[default]
    Whole,
    /// The content is cut every `size` bytes.
    Fixed {
        /// How many bytes each chunk holds, bar the last.
        size: u32,
    },
    /// The content is cut where the content says so.
    Cdc {
        /// The smallest a chunk may be.
        min: u32,
        /// The size a chunk tends towards.
        average: u32,
        /// The largest a chunk may be.
        max: u32,
    },
    /// The content is cut at the ends of its lines, so that what a chunk holds is whole lines.
    ///
    /// This is the cut for text — see [`Text`](crate::Text) — and it is what makes an edit to a
    /// few lines cost those lines rather than the file: the lines around them are the same bytes
    /// they were, so they are chunks the store already has.
    Text {
        /// The smallest a chunk may be.
        min: u32,
        /// The size a chunk tends towards.
        average: u32,
        /// The largest a chunk may be.
        max: u32,
    },
    /// The content is cut at the boundaries of the members it is packed of.
    ///
    /// This is the cut for content that is a container of other content — a `.zip`, and what
    /// `.docx`, `.pptx`, `.xlsx`, `.odt`, `.jar` and `.apk` are — see [`Zip`](crate::Zip). Cutting
    /// there rather than on content is what makes an edit to one member leave every member it did
    /// not touch exactly as it was: a member the writer did not rewrite is a chunk the store
    /// already has, so the edit adds one chunk rather than the whole container.
    ///
    /// Nothing is unpacked: a member is stored as the bytes it already is, so what comes back out
    /// is what went in byte for byte, and a member that would unpack to something enormous is never
    /// opened to find out.
    Zip,
}

impl Chunker for Chunking {
    fn split(&self, content: &[u8]) -> Vec<Range<usize>> {
        match *self {
            Self::Whole => Whole.split(content),
            Self::Fixed { size } => Fixed::new(size).split(content),
            Self::Cdc { min, average, max } => Cdc::new(min, average, max).split(content),
            Self::Text { min, average, max } => Text::new(min, average, max).split(content),
            Self::Zip => crate::Zip.split(content),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use super::{Cdc, Chunker as _, Chunking, Fixed, Text, Whole};

    /// Content that does not repeat itself, so a cut can only come from the bytes.
    fn content(len: usize) -> Vec<u8> {
        let mut state = 0x1234_5678_9ABC_DEF0_u64;

        (0..len)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                state.to_le_bytes()[4]
            })
            .collect()
    }

    /// Whether the ranges are in order, do not overlap, and cover `len` bytes exactly.
    fn covers(ranges: &[Range<usize>], len: usize) -> bool {
        let mut at = 0;

        for range in ranges {
            if range.start != at || range.end < range.start {
                return false;
            }
            at = range.end;
        }

        at == len
    }

    #[test]
    fn the_whole_chunker_covers_everything_in_one_chunk() {
        let ranges = Whole.split(b"abc");

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], 0..3);
    }

    #[test]
    fn a_fixed_chunker_cuts_at_the_size_and_keeps_the_last_short() {
        let ranges = Fixed::new(4).split(b"0123456789");
        let sizes: Vec<usize> = ranges.iter().map(Range::len).collect();

        assert_eq!(sizes, [4, 4, 2]);
        assert!(covers(&ranges, 10));
    }

    #[test]
    fn a_content_defined_chunker_cuts_within_its_bounds_and_covers_everything() {
        let content = content(100_000);
        let ranges = Cdc::new(64, 256, 1024).split(&content);

        assert!(covers(&ranges, content.len()));
        for range in &ranges {
            assert!(range.len() >= 64, "{range:?}");
            assert!(range.len() <= 1024, "{range:?}");
        }
    }

    #[test]
    fn a_content_defined_chunker_finds_the_same_cuts_twice() {
        let content = content(4096);
        let chunker = Cdc::new(32, 128, 512);

        // A chunker that answered differently on another day would give one file two manifests.
        assert_eq!(chunker.split(&content), chunker.split(&content));
    }

    #[test]
    fn an_insertion_only_disturbs_the_chunks_it_lands_between() {
        let content = content(50_000);
        let chunker = Cdc::new(64, 256, 1024);

        let before: Vec<&[u8]> = chunker
            .split(&content)
            .iter()
            .map(|range| &content[range.clone()])
            .collect();

        let mut edited = content.clone();
        edited.insert(25_000, 0xAB);
        let after: Vec<&[u8]> = chunker
            .split(&edited)
            .iter()
            .map(|range| &edited[range.clone()])
            .collect();

        // Cutting on content is what makes an edit cheap: everything before it is untouched, and
        // everything after lines up again, so only the chunk the insertion landed in is new.
        let kept = before.iter().filter(|chunk| after.contains(chunk)).count();

        assert!(
            kept + 2 >= before.len(),
            "{kept} of {} chunks kept",
            before.len()
        );
    }

    /// Text of the lines from `from` up to `to`, which is what a cut for text has to cut.
    fn lines(from: usize, to: usize) -> Vec<u8> {
        use std::fmt::Write as _;

        let mut text = String::new();
        for line in from..to {
            writeln!(text, "line {line}").expect("writing to a `String` cannot fail");
        }

        text.into_bytes()
    }

    #[test]
    fn a_text_chunker_cuts_at_the_ends_of_lines() {
        let content = lines(0, 2000);
        let ranges = Text::new(64, 256, 1024).split(&content);

        assert!(covers(&ranges, content.len()));
        // Every chunk but the last ends where a line ends, which is what makes each of them a
        // whole number of lines.
        for range in &ranges {
            if range.end < content.len() {
                assert_eq!(content[range.end - 1], b'\n', "{range:?}");
            }
            assert!(range.len() >= 64, "{range:?}");
            // A cut moved on to the next line end may overshoot `max` by the line it moved over,
            // and no line here is anywhere near this long.
            assert!(range.len() <= 1024 + 32, "{range:?}");
        }
    }

    #[test]
    fn a_line_an_edit_did_not_touch_is_the_same_chunk() {
        let before = lines(0, 2000);

        // The same lines, with one added in the middle of them.
        let mut after = lines(0, 1000);
        after.extend_from_slice(b"a line that was added\n");
        after.extend_from_slice(&lines(1000, 2000));

        let chunks = |content: &[u8]| -> Vec<Vec<u8>> {
            Text::new(64, 256, 1024)
                .split(content)
                .iter()
                .map(|range| content[range.clone()].to_vec())
                .collect()
        };
        let before = chunks(&before);
        let after = chunks(&after);

        let shared = before.iter().filter(|chunk| after.contains(chunk)).count();

        // Lines are what an edit moves, so only the lines it landed between come out new.
        assert!(shared + 2 >= before.len(), "{shared} of {}", before.len());
    }

    #[test]
    fn every_chunking_cut_covers_the_content() {
        let content = b"the same bytes, cut four ways";

        for chunking in [
            Chunking::Whole,
            Chunking::Fixed { size: 5 },
            Chunking::Cdc {
                min: 4,
                average: 8,
                max: 16,
            },
            Chunking::Text {
                min: 4,
                average: 8,
                max: 16,
            },
            Chunking::Zip,
        ] {
            assert!(
                covers(&chunking.split(content), content.len()),
                "{chunking:?}"
            );
        }
    }

    #[test]
    fn an_empty_content_is_cut_into_nothing() {
        assert!(Chunking::Fixed { size: 4 }.split(b"").is_empty());
        assert!(
            Chunking::Cdc {
                min: 4,
                average: 8,
                max: 16,
            }
            .split(b"")
            .is_empty()
        );
    }
}
