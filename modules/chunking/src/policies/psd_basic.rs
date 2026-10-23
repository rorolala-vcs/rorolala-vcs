//! A strategy that cuts a Photoshop document at the boundaries of what it holds.
//!
//! A PSD is one file holding several things side by side: a colour mode block, image resources, the
//! layer and mask block — layer records, then each layer's channel bytes — and the composite image.
//! A save rewrites the file from its first byte, so a cut by position moves everything after the
//! first edit. A cut at these inner boundaries keeps what the save did not touch: the channel bytes
//! an edit to one layer leaves alone, the composite a metadata change leaves alone. Those are the
//! chunks a store already has.
//!
//! Nothing is decompressed. A channel is stored as the bytes it already is, so what comes back out
//! is what went in, byte for byte. What cannot be read as a PSD this build understands is not taken
//! apart: it is cut into one piece, which is always a correct answer.

use std::ops::Range;

use crate::{Chunker, Policy, Whole};

/// The signature a Photoshop document starts with.
const SIGNATURE: [u8; 4] = *b"8BPS";

/// The header version this reads: version 1 is PSD.
///
/// Version 2 is PSB, which writes the layer and mask lengths eight bytes wide and stores channels
/// beyond what a PSD can hold. Reading one as a PSD would put every boundary after the first wrong
/// length in the wrong place, so a PSB is cut whole instead.
const VERSION: u16 = 1;

/// Where the version sits in the header.
const VERSION_AT: usize = 4;

/// The bytes before the first length-prefixed section.
const HEADER: usize = 26;

/// How many layers' channel bytes one chunk carries.
///
/// A manifest record costs a fixed number of bytes per chunk, and a layer's channels average a few
/// tens of kilobytes, so one chunk per layer would spend a noticeable share of the store on records.
/// Sixteen layers is a few hundred kilobytes of channel data in the documents this was measured on:
/// large enough that the manifest is a fraction of a percent, small enough that an edit to one layer
/// does not rewrite everything beside it.
const LAYERS_PER_CHUNK: usize = 16;

/// A strategy that cuts a Photoshop document at the boundaries of what it holds.
///
/// The spans it keeps apart are the colour mode data, the image resources, the layer records, the
/// channel data of each run of `LAYERS_PER_CHUNK` layers — and the composite image.
/// Everything else is a header, a length or padding, and falls between them as a range of its own.
///
/// The records are one span, not one per layer. A save writes the modification time of every layer
/// into its record, so every record changes on every save and no cut of them deduplicates across
/// versions; cutting them apart would only pay a manifest record for each. The channel data beside
/// them is the part a save usually leaves alone, and the layers of one run are one span because the
/// store pays for a manifest record per chunk: a chunk per layer is more record than content.
///
/// Splitting the record itself would gain a few hundred bytes and lose the guarantee that what is
/// read is what a writer wrote.
#[derive(Debug, Clone, Copy, Default)]
pub struct PsdBasic;

impl Chunker for PsdBasic {
    fn split(&self, content: &[u8]) -> Vec<Range<usize>> {
        document(content).map_or_else(
            || Whole.split(content),
            |spans| crate::spans::covering(content.len(), spans),
        )
    }
}

impl Policy for PsdBasic {
    const NAME: &'static str = "psd-basic";
    const EXTENSIONS: &'static [&'static str] = &["psd"];
}

/// The spans the document's contents take up, or `None` when it does not read as a PSD.
fn document(content: &[u8]) -> Option<Vec<(usize, usize)>> {
    let header = content.get(..HEADER)?;
    if !header.starts_with(&SIGNATURE) || read_u16(header, VERSION_AT)? != VERSION {
        return None;
    }

    let mut spans = Vec::new();
    let mut at = HEADER;

    // Colour mode data, then image resources: both are one length-prefixed section taken whole.
    at = section(content, at, &mut spans)?;
    at = section(content, at, &mut spans)?;
    at = layers(content, at, &mut spans)?;
    image(content, at, &mut spans)?;

    Some(spans)
}

/// Takes one length-prefixed section whole, and returns where it ends.
fn section(content: &[u8], at: usize, spans: &mut Vec<(usize, usize)>) -> Option<usize> {
    let length = usize::try_from(read_u32(content, at)?).ok()?;
    let body = at.checked_add(4)?;
    let end = advance(content, body, length, content.len())?;

    if body < end {
        spans.push((body, end));
    }

    Some(end)
}

/// Reads the layer and mask section: every layer record, then every layer's channels.
///
/// What follows the last channel — the global layer mask and the additional information blocks — is
/// not taken apart. It is small and it belongs to no layer, so it falls between the channel spans
/// and the composite image as a range of its own.
fn layers(content: &[u8], at: usize, spans: &mut Vec<(usize, usize)>) -> Option<usize> {
    let length = usize::try_from(read_u32(content, at)?).ok()?;
    let body = at.checked_add(4)?;
    let end = advance(content, body, length, content.len())?;

    // A flattened document may have no layer block at all, and then there is nothing to read.
    if length > 0 {
        let info_length = usize::try_from(read_u32(content, body)?).ok()?;
        let info = body.checked_add(4)?;
        let info_end = advance(content, info, info_length, end)?;
        layer_info(content, info, info_end, spans)?;
    }

    Some(end)
}

/// Reads the layer records and the channel data that follows them.
///
/// The records come first and in order; the channel data follows them in the same order, each
/// layer taking the lengths its record gave its channels.
fn layer_info(
    content: &[u8],
    at: usize,
    end: usize,
    spans: &mut Vec<(usize, usize)>,
) -> Option<()> {
    let count = read_i16(content, at)?.unsigned_abs();
    let mut at = at.checked_add(2)?;

    // One entry per layer: the bytes its channels take together, in the order it named them.
    let first = at;
    let mut lengths = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        let (after, channels) = record(content, at, end)?;
        lengths.push(
            channels
                .iter()
                .try_fold(0_usize, |sum, length| sum.checked_add(*length))?,
        );
        at = after;
    }

    // Every record is one span: a save writes a modification time into each of them, so they all
    // change together and cutting them apart would only pay a manifest record for each.
    if at > first {
        spans.push((first, at));
    }

    // The channel bytes of every layer, as the pair each one takes up.
    let mut data = Vec::with_capacity(lengths.len());
    for length in lengths {
        let after = advance(content, at, length, end)?;
        data.push((at, after));
        at = after;
    }

    for group in data.chunks(LAYERS_PER_CHUNK) {
        if let (Some(first), Some(last)) = (group.first(), group.last())
            && first.0 < last.1
        {
            spans.push((first.0, last.1));
        }
    }

    // A writer may pad the layer information by a byte or three so that what follows starts where
    // it likes. Those bytes belong to no span and are covered as the gap they are.
    Some(())
}

/// Reads one layer record, returning where it ends and how long each of its channels is.
///
/// Only the fixed fields before the extra data are read. What the extra data holds — the layer name,
/// its mask, its blending ranges, its own information blocks — is skipped by the length that says
/// how much of it there is, because none of it says where the next record or the channel data begins.
fn record(content: &[u8], at: usize, limit: usize) -> Option<(usize, Vec<usize>)> {
    let channel_count = read_u16(content, at.checked_add(16)?)?;
    let mut at = at.checked_add(18)?;

    let mut lengths = Vec::new();
    for _ in 0..channel_count {
        // Each channel is an id and the length of its data, which includes its compression word.
        lengths.push(usize::try_from(read_u32(content, at.checked_add(2)?)?).ok()?);
        at = at.checked_add(6)?;
    }

    // The blend mode, opacity, clipping and flags take twelve bytes; the length of the extra data
    // and the extra data itself follow it.
    let extra = usize::try_from(read_u32(content, at.checked_add(12)?)?).ok()?;
    let end = advance(content, at.checked_add(16)?, extra, limit)?;

    Some((end, lengths))
}

/// Takes the composite image, and returns where the document ends.
///
/// The compression word belongs to no span: what is cut out is the bytes after it, so a document
/// whose compression changed but whose pixels did not is a chunk the store already has. The image
/// runs to the end of the file, which is where a PSD puts it.
fn image(content: &[u8], at: usize, spans: &mut Vec<(usize, usize)>) -> Option<()> {
    // Reading the word is also the check that the image section is there at all.
    read_u16(content, at)?;

    let body = at.checked_add(2)?;
    if body < content.len() {
        spans.push((body, content.len()));
    }

    Some(())
}

/// Where `length` bytes after `at` end, when that is inside `content` and no further than `limit`.
fn advance(content: &[u8], at: usize, length: usize, limit: usize) -> Option<usize> {
    let end = at.checked_add(length)?;
    content.get(..end)?;
    (end <= limit).then_some(end)
}

/// Two big-endian bytes at `at`.
fn read_u16(content: &[u8], at: usize) -> Option<u16> {
    let bytes = content.get(at..at.checked_add(2)?)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

/// Two big-endian bytes at `at`, signed.
fn read_i16(content: &[u8], at: usize) -> Option<i16> {
    let bytes = content.get(at..at.checked_add(2)?)?;
    Some(i16::from_be_bytes([bytes[0], bytes[1]]))
}

/// Four big-endian bytes at `at`.
fn read_u32(content: &[u8], at: usize) -> Option<u32> {
    let bytes = content.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use super::{LAYERS_PER_CHUNK, PsdBasic};
    use crate::Chunker as _;

    /// A channel's stored bytes: the compression word and what follows it.
    fn channel(bytes: &[u8]) -> Vec<u8> {
        let mut data = vec![0, 0];
        data.extend_from_slice(bytes);
        data
    }

    /// A layer's extra data: no mask, no blending ranges, and a name padded to four bytes.
    fn extra(name: &str) -> Vec<u8> {
        let mut extra = Vec::new();
        extra.extend_from_slice(&0_u32.to_be_bytes());
        extra.extend_from_slice(&0_u32.to_be_bytes());

        let name = name.as_bytes();
        extra.push(u8::try_from(name.len()).unwrap());
        extra.extend_from_slice(name);
        while (extra.len() - 8) % 4 != 0 {
            extra.push(0);
        }

        extra
    }

    /// One layer record: its rectangle is zero and every channel is numbered as a PSD numbers one.
    fn record(name: &str, channels: &[Vec<u8>]) -> Vec<u8> {
        let mut record = vec![0_u8; 16];
        record.extend_from_slice(&u16::try_from(channels.len()).unwrap().to_be_bytes());

        for (index, data) in channels.iter().enumerate() {
            // The first channel of a layer is its transparency, and -1 is what PSD calls it.
            let id = if index == 0 {
                -1
            } else {
                i16::try_from(index - 1).unwrap()
            };
            record.extend_from_slice(&id.to_be_bytes());
            record.extend_from_slice(&u32::try_from(data.len()).unwrap().to_be_bytes());
        }

        record.extend_from_slice(b"8BIMnorm");
        record.extend_from_slice(&[255, 0, 0, 0]);

        let extra = extra(name);
        record.extend_from_slice(&u32::try_from(extra.len()).unwrap().to_be_bytes());
        record.extend_from_slice(&extra);

        record
    }

    /// A layer record's channels as they are stored: after every record, in layer order.
    fn layer_block(layers: &[(&str, Vec<Vec<u8>>)]) -> Vec<u8> {
        let mut records = Vec::new();
        let mut channels = Vec::new();

        for (name, data) in layers {
            records.extend_from_slice(&record(name, data));
            for channel in data {
                channels.extend_from_slice(channel);
            }
        }

        let mut info = Vec::new();
        info.extend_from_slice(&i16::try_from(layers.len()).unwrap().to_be_bytes());
        info.extend_from_slice(&records);
        info.extend_from_slice(&channels);

        info
    }

    /// A PSD holding `layers` and a composite of `image`.
    fn document(layers: &[(&str, Vec<Vec<u8>>)], image: &[u8]) -> Vec<u8> {
        let mut content = Vec::new();
        content.extend_from_slice(b"8BPS");
        content.extend_from_slice(&1_u16.to_be_bytes()); // version
        content.extend_from_slice(&[0; 6]); // reserved
        content.extend_from_slice(&3_u16.to_be_bytes()); // channels
        content.extend_from_slice(&1_u32.to_be_bytes()); // height
        content.extend_from_slice(&1_u32.to_be_bytes()); // width
        content.extend_from_slice(&8_u16.to_be_bytes()); // depth
        content.extend_from_slice(&3_u16.to_be_bytes()); // RGB

        content.extend_from_slice(&0_u32.to_be_bytes()); // colour mode data

        let resources = b"resources";
        content.extend_from_slice(&u32::try_from(resources.len()).unwrap().to_be_bytes());
        content.extend_from_slice(resources);

        let info = layer_block(layers);
        // The section is the layer info and the global mask, with a length before each.
        let length = 4 + info.len() + 4;
        content.extend_from_slice(&u32::try_from(length).unwrap().to_be_bytes());
        content.extend_from_slice(&u32::try_from(info.len()).unwrap().to_be_bytes());
        content.extend_from_slice(&info);
        content.extend_from_slice(&0_u32.to_be_bytes()); // global layer mask

        content.extend_from_slice(&0_u16.to_be_bytes()); // composite compression
        content.extend_from_slice(image);

        content
    }

    /// A document's chunks, as whole values.
    fn chunks(content: &[u8]) -> Vec<Vec<u8>> {
        PsdBasic
            .split(content)
            .iter()
            .map(|range| content[range.clone()].to_vec())
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
    fn a_document_is_cut_at_the_boundaries_of_what_it_holds() {
        let content = document(
            &[
                ("first", vec![channel(b"the first layer"), channel(b"red")]),
                ("second", vec![channel(b"the second layer")]),
            ],
            b"the composite",
        );

        let ranges = PsdBasic.split(&content);
        assert!(covers(&ranges, content.len()));

        let chunks = chunks(&content);
        // The layers' channels are one chunk, and so is the composite after its compression word.
        let mut group = channel(b"the first layer");
        group.extend_from_slice(&channel(b"red"));
        group.extend_from_slice(&channel(b"the second layer"));
        assert!(chunks.contains(&group), "{chunks:?}");
        assert!(chunks.contains(&b"the composite".to_vec()), "{chunks:?}");
    }

    #[test]
    fn an_edit_to_one_layer_leaves_the_other_run_the_same_chunk() {
        // Two runs of layers, so the edit lands in the second one and the first is untouched.
        let data = |index: usize| {
            let bytes = vec![u8::try_from(index).unwrap(); 32 + index];
            channel(&bytes)
        };
        let mut layers: Vec<(&str, Vec<Vec<u8>>)> = (0..LAYERS_PER_CHUNK * 2)
            .map(|index| ("layer", vec![data(index)]))
            .collect();
        layers[LAYERS_PER_CHUNK + 1].1 = vec![channel(b"a changed layer")];

        let after = chunks(&document(&layers, b"the composite"));

        // What the edit did not touch is the bytes it already was, so it is a chunk the store
        // already has — which is the whole reason to cut a document at its channels.
        let mut untouched = Vec::new();
        for index in 0..LAYERS_PER_CHUNK {
            untouched.extend_from_slice(&data(index));
        }
        assert!(after.contains(&untouched), "{after:?}");
        assert!(after.contains(&b"the composite".to_vec()), "{after:?}");
    }

    #[test]
    fn a_renamed_layer_leaves_every_channel_the_same_chunk() {
        let after = document(
            &[
                ("renamed", vec![channel(b"the first layer")]),
                ("second", vec![channel(b"the second layer")]),
            ],
            b"the composite",
        );

        let after = chunks(&after);

        // A name lives in the record, and the record is cut apart from the channels.
        let mut group = channel(b"the first layer");
        group.extend_from_slice(&channel(b"the second layer"));
        assert!(after.contains(&group), "{after:?}");
    }

    #[test]
    fn every_cut_comes_to_the_whole_of_what_it_was_given() {
        let content = document(
            &[
                ("first", vec![channel(&[7_u8; 300])]),
                ("second", vec![channel(&[9_u8; 50])]),
            ],
            &[3_u8; 4096],
        );

        let ranges = PsdBasic.split(&content);
        let put_back: Vec<u8> = ranges
            .iter()
            .flat_map(|range| &content[range.clone()])
            .copied()
            .collect();

        assert_eq!(put_back, content);
    }

    #[test]
    fn the_same_document_is_cut_the_same_way_twice() {
        let content = document(&[("first", vec![channel(b"layer")])], b"image");

        assert_eq!(PsdBasic.split(&content), PsdBasic.split(&content));
    }

    #[test]
    fn what_does_not_read_as_a_document_is_one_chunk() {
        let content = document(&[("first", vec![channel(b"layer")])], b"image");

        // A truncated file, a body that is not a PSD at all, and a section length that runs past
        // the content are the shapes a writer that stopped halfway leaves.
        let mut overlong = content.clone();
        let at = 26 + 4; // the length of the colour mode data
        overlong[at..at + 4].copy_from_slice(&u32::MAX.to_be_bytes());

        for content in [
            b"not a PSD at all".as_slice(),
            b"8BPS",
            &content[..content.len() / 2],
            &overlong,
            b"",
        ] {
            let ranges = PsdBasic.split(content);

            assert_eq!(ranges.len(), 1, "{ranges:?}");
            assert!(covers(&ranges, content.len()));
        }
    }

    #[test]
    fn a_version_this_does_not_read_is_one_chunk() {
        let mut content = document(&[("first", vec![channel(b"layer")])], b"image");
        // PSB stores its lengths differently, so reading it as a PSD would be wrong.
        content[4..6].copy_from_slice(&2_u16.to_be_bytes());

        assert_eq!(PsdBasic.split(&content).len(), 1);
    }
}
