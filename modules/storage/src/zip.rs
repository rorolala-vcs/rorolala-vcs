//! Cutting a packed container at the boundaries of what it packs.
//!
//! Some content is not one thing but many, packed one after another: a `.docx`, a `.pptx`, a
//! `.xlsx`, an `.odt`, a `.jar`, an `.apk` are all a ZIP of members, and so is a plain `.zip`.
//!
//! Cutting such content on its content — see [`Cdc`](crate::Cdc) — finds little, because what a
//! ZIP holds is mostly the output of a compressor, which shares no bytes with the same content
//! compressed again. Cutting it *at its member boundaries* finds a great deal: a member the writer
//! did not touch is the same bytes in the same order it was, so it is a chunk the store already
//! has, and an edit to one member of a pack adds one chunk rather than the whole pack.
//!
//! Nothing here unpacks anything. A member is stored as the bytes it already is — its header, its
//! name and its compressed data — so what comes back out is what went in, byte for byte, without
//! anything having to reproduce a compressor's choices, and without a bomb in a member ever being
//! opened to find out how large it would have been.
//!
//! What cannot be read as a container this build understands is not taken apart at all: it is cut
//! into one piece, which is always a correct answer. Cutting wrongly would not be.

use std::ops::Range;

use crate::{Chunker, Whole};

/// The signature a member's local header starts with, which is also what a ZIP starts with.
const LOCAL_MAGIC: [u8; 4] = *b"PK\x03\x04";

/// The signature a member's record in the central directory starts with.
const CENTRAL_MAGIC: [u8; 4] = *b"PK\x01\x02";

/// The signature the end of the central directory starts with.
const END_MAGIC: [u8; 4] = *b"PK\x05\x06";

/// The signature a ZIP starts with.
///
/// It is what a write looks at to tell a packed container from content of any other kind.
pub const ZIP_MAGIC: [u8; 4] = LOCAL_MAGIC;

/// How long a member's local header is, before its name and its extra field.
const LOCAL_HEADER: usize = 30;

/// How long a member's record in the central directory is, before its name, extra field and
/// comment.
const CENTRAL_HEADER: usize = 46;

/// How long the end of the central directory is, before the comment that may follow it.
const END_RECORD: usize = 22;

/// How far back the end of the central directory may sit, given the comment after it.
///
/// A comment is as long as a `u16` says, which is what makes this the furthest the end record can
/// be from the end of the content.
const MAX_COMMENT: usize = 65_535;

/// A chunker that cuts a packed container at the boundaries of what it packs.
///
/// It reads the central directory — which is written for the sake of being read, and whose offsets
/// survive a member being written with sizes its header leaves blank — and takes each member as
/// the range from its local header to the end of its data. What follows the last member is the
/// central directory and its end record, and they are one range: an edit moves every offset in
/// them, so there is nothing in them to hold on to.
///
/// Content that does not read as a container this build can take apart is cut into one piece
/// rather than refused, so nothing about it changes but the number of chunks it becomes.
#[derive(Debug, Clone, Copy, Default)]
pub struct Zip;

impl Chunker for Zip {
    fn split(&self, content: &[u8]) -> Vec<Range<usize>> {
        packed(content).unwrap_or_else(|| Whole.split(content))
    }
}

/// The ranges a packed container is cut into, or `None` when it does not read as one.
fn packed(content: &[u8]) -> Option<Vec<Range<usize>>> {
    let (directory_at, count) = directory(content)?;
    let spans = members(content, directory_at, count)?;

    Some(ranges(content.len(), spans))
}

/// Where the central directory is, and how many members it says there are.
fn directory(content: &[u8]) -> Option<(usize, usize)> {
    let earliest = content.len().saturating_sub(END_RECORD + MAX_COMMENT);
    let latest = content.len().checked_sub(END_RECORD)?;

    // The end of the central directory is the last thing a container holds but for its comment, so
    // the search starts at the end and walks back: a member's data may hold these very bytes.
    for at in (earliest..=latest).rev() {
        let Some(record) = content.get(at..at + END_RECORD) else {
            continue;
        };
        if !record.starts_with(&END_MAGIC) {
            continue;
        }

        // What is found is the end record only when what it says is left after it is all there is.
        let comment = usize::from(u16::from_le_bytes([record[20], record[21]]));
        if at + END_RECORD + comment != content.len() {
            continue;
        }

        let count = usize::from(u16::from_le_bytes([record[10], record[11]]));
        let size = u32::from_le_bytes([record[12], record[13], record[14], record[15]]);
        let offset = u32::from_le_bytes([record[16], record[17], record[18], record[19]]);

        // A number left as all ones is a `zip64` record's to say, and this build does not read
        // those; nor does it read a directory that claims to sit outside the content.
        if size == u32::MAX || offset == u32::MAX || count == usize::from(u16::MAX) {
            return None;
        }

        let offset = usize::try_from(offset).ok()?;
        content.get(offset..)?;

        return Some((offset, count));
    }

    None
}

/// The span each member takes up, in the order they sit in the content.
fn members(content: &[u8], directory: usize, count: usize) -> Option<Vec<(usize, usize)>> {
    let mut spans = Vec::with_capacity(count);
    let mut at = directory;

    for _ in 0..count {
        let record = content.get(at..at + CENTRAL_HEADER)?;
        if !record.starts_with(&CENTRAL_MAGIC) {
            return None;
        }

        let compressed = u32::from_le_bytes([record[20], record[21], record[22], record[23]]);
        let name = usize::from(u16::from_le_bytes([record[28], record[29]]));
        let extra = usize::from(u16::from_le_bytes([record[30], record[31]]));
        let comment = usize::from(u16::from_le_bytes([record[32], record[33]]));
        let local = u32::from_le_bytes([record[42], record[43], record[44], record[45]]);

        // A size a `zip64` extra field really holds is one this build does not know how long is.
        if compressed == u32::MAX {
            return None;
        }

        spans.push(span(
            content,
            usize::try_from(local).ok()?,
            usize::try_from(compressed).ok()?,
        )?);
        at = at.checked_add(CENTRAL_HEADER + name + extra + comment)?;
    }

    // The directory may name members in an order other than the one they sit in.
    spans.sort_unstable();

    Some(spans)
}

/// The span the member whose local header is at `start` takes up.
///
/// What is cut out is the member's **data**, not its header: a header carries the time it was
/// written and where it sits, which a writer may change without the content changing — so a member
/// whose bytes are the same as they were is a chunk the store already has, even after a save that
/// touched every header in the container. Nothing is lost by leaving the headers out: they fall
/// between the members, and what lies between two members is a range of its own.
fn span(content: &[u8], start: usize, compressed: usize) -> Option<(usize, usize)> {
    let header = content.get(start..start + LOCAL_HEADER)?;
    if !header.starts_with(&LOCAL_MAGIC) {
        return None;
    }

    let name = usize::from(u16::from_le_bytes([header[26], header[27]]));
    let extra = usize::from(u16::from_le_bytes([header[28], header[29]]));

    let data = start.checked_add(LOCAL_HEADER + name + extra)?;
    let end = data.checked_add(compressed)?;
    content.get(..end)?;

    Some((data, end))
}

/// The spans turned into ranges that cover `len` exactly and in order.
///
/// A container may hold bytes belonging to no member — a descriptor between one member and the
/// next, or whatever a writer left behind — and what belongs to no member is a range of its own, so
/// that what comes back out is always all of what went in.
fn ranges(len: usize, spans: Vec<(usize, usize)>) -> Vec<Range<usize>> {
    let mut ranges = Vec::with_capacity(spans.len() + 1);
    let mut at = 0;

    for (start, end) in spans {
        let start = start.max(at);
        let end = end.max(start).min(len);

        if start > at {
            ranges.push(at..start);
        }
        if end > start {
            ranges.push(start..end);
        }

        at = end;
    }

    // Whatever is left over is the central directory and its end record, and they are cut as one.
    if at < len {
        ranges.push(at..len);
    }

    ranges
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use super::{
        CENTRAL_HEADER, Chunker as _, END_MAGIC, END_RECORD, LOCAL_HEADER, ZIP_MAGIC, Zip,
    };

    /// A packed container of `members`, stored rather than deflated and stamped with `time`.
    ///
    /// This is the shape a chunker reads, which is what is being tested, rather than the shape a
    /// packer would choose — and the stamp is what a save that changed nothing else moves.
    fn packed_at(members: &[(&str, &[u8])], stamp: u16) -> Vec<u8> {
        let mut local = Vec::new();
        let mut directory = Vec::new();

        for (name, data) in members {
            let name = name.as_bytes();
            let size = u32::try_from(data.len()).unwrap();
            let length = u16::try_from(name.len()).unwrap();
            let offset = u32::try_from(local.len()).unwrap();

            let mut header = Vec::new();
            header.extend_from_slice(&ZIP_MAGIC); // signature
            header.extend_from_slice(&[20, 0]); // version needed
            header.extend_from_slice(&[0, 0]); // flags
            header.extend_from_slice(&[0, 0]); // method
            header.extend_from_slice(&stamp.to_le_bytes()); // time
            header.extend_from_slice(&[0, 0]); // date
            header.extend_from_slice(&[0, 0, 0, 0]); // crc
            header.extend_from_slice(&size.to_le_bytes()); // compressed
            header.extend_from_slice(&size.to_le_bytes()); // uncompressed
            header.extend_from_slice(&length.to_le_bytes()); // name
            header.extend_from_slice(&[0, 0]); // extra
            header.extend_from_slice(name);
            assert_eq!(header.len(), LOCAL_HEADER + name.len());

            local.extend_from_slice(&header);
            local.extend_from_slice(data);

            let mut record = Vec::new();
            record.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]); // signature
            record.extend_from_slice(&[20, 0]); // made by
            record.extend_from_slice(&[20, 0]); // needed
            record.extend_from_slice(&[0, 0]); // flags
            record.extend_from_slice(&[0, 0]); // method
            record.extend_from_slice(&stamp.to_le_bytes()); // time
            record.extend_from_slice(&[0, 0]); // date
            record.extend_from_slice(&[0, 0, 0, 0]); // crc
            record.extend_from_slice(&size.to_le_bytes()); // compressed
            record.extend_from_slice(&size.to_le_bytes()); // uncompressed
            record.extend_from_slice(&length.to_le_bytes()); // name
            record.extend_from_slice(&[0, 0]); // extra
            record.extend_from_slice(&[0, 0]); // comment
            record.extend_from_slice(&[0, 0]); // disk
            record.extend_from_slice(&[0, 0]); // internal
            record.extend_from_slice(&[0, 0, 0, 0]); // external
            record.extend_from_slice(&offset.to_le_bytes()); // where its header is
            record.extend_from_slice(name);
            assert_eq!(record.len(), CENTRAL_HEADER + name.len());

            directory.extend_from_slice(&record);
        }

        let mut end = Vec::new();
        end.extend_from_slice(&END_MAGIC);
        end.extend_from_slice(&[0, 0]); // disk
        end.extend_from_slice(&[0, 0]); // directory's disk
        end.extend_from_slice(&u16::try_from(members.len()).unwrap().to_le_bytes()); // here
        end.extend_from_slice(&u16::try_from(members.len()).unwrap().to_le_bytes()); // in all
        end.extend_from_slice(&u32::try_from(directory.len()).unwrap().to_le_bytes());
        end.extend_from_slice(&u32::try_from(local.len()).unwrap().to_le_bytes());
        end.extend_from_slice(&[0, 0]); // comment
        assert_eq!(end.len(), END_RECORD);

        let mut content = local;
        content.extend_from_slice(&directory);
        content.extend_from_slice(&end);

        content
    }

    /// A packed container whose members were written at time zero.
    fn packed(members: &[(&str, &[u8])]) -> Vec<u8> {
        packed_at(members, 0)
    }

    /// A packed container's chunks, as whole values.
    fn chunks(archive: &[u8]) -> Vec<Vec<u8>> {
        Zip.split(archive)
            .iter()
            .map(|range| archive[range.clone()].to_vec())
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
    fn a_container_is_cut_at_the_boundaries_of_what_it_packs() {
        let archive = packed(&[("one", b"the first member"), ("two", b"the second member")]);
        let ranges = Zip.split(&archive);

        assert!(covers(&ranges, archive.len()));
        // A header and a member's bytes for each of the two members, and the directory after them.
        assert_eq!(ranges.len(), 5, "{ranges:?}");

        // What is cut out is the member itself: the range after its header is its bytes, and
        // nothing of the member after it.
        let header = ranges[0].end;
        assert_eq!(header, LOCAL_HEADER + 3);
        assert_eq!(&archive[ranges[1].clone()], b"the first member");
    }

    #[test]
    fn a_member_an_edit_did_not_touch_is_the_same_chunk() {
        let before = packed(&[("one", b"the first member"), ("two", b"the second member")]);
        let after = packed(&[("one", b"the first member"), ("two", b"the SECOND member")]);

        let before = chunks(&before);
        let after = chunks(&after);

        // What the edit did not touch is the bytes it already was, so it is a chunk the store
        // already has — which is the whole reason to cut a container at its members.
        assert_eq!(before[1], after[1]);
        assert_ne!(before[3], after[3]);
    }

    #[test]
    fn a_member_is_the_same_chunk_after_a_save_that_moved_its_time() {
        let before = packed_at(
            &[("one", b"the first member"), ("two", b"the second member")],
            0x0000,
        );
        let after = packed_at(
            &[("one", b"the first member"), ("two", b"the second member")],
            0x1234,
        );

        let shared = chunks(&before)
            .into_iter()
            .filter(|chunk| chunks(&after).contains(chunk))
            .count();

        // Every header says a different time and the members say the same bytes: the bytes are the
        // chunks that are shared, which is what makes a save that only moved the times cheap.
        assert_eq!(shared, 2, "{shared} chunks are shared");
    }

    #[test]
    fn what_does_not_read_as_a_container_is_one_chunk() {
        let archive = packed(&[("one", b"the first member")]);
        let without_end = &archive[..archive.len() - END_RECORD];

        for content in [
            b"not a container at all".as_slice(),
            b"PK\x03\x04 and then nothing that follows",
            &archive[..archive.len() / 2],
            without_end,
            b"",
        ] {
            let ranges = Zip.split(content);

            assert_eq!(ranges.len(), 1, "{ranges:?}");
            assert!(covers(&ranges, content.len()));
        }
    }

    #[test]
    fn every_cut_comes_to_the_whole_of_what_it_was_given() {
        let archive = packed(&[
            ("one", b"the first member"),
            ("two", b"the second member"),
            ("three", &[7_u8; 300]),
        ]);
        let ranges = Zip.split(&archive);

        let put_back: Vec<u8> = ranges
            .iter()
            .flat_map(|range| &archive[range.clone()])
            .copied()
            .collect();

        assert_eq!(put_back, archive);
    }
}
