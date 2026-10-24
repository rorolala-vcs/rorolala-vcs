//! Cut Blender file blocks where their unchanged payloads survive a save.

use std::ops::Range;

use crate::{Chunker, Policy, Whole};

/// Keep small records and their changing headers together to limit manifest overhead.
const MIN_PAYLOAD: usize = 1024;

/// The block header layout written in a Blender file header.
#[derive(Clone, Copy)]
enum Layout {
    Legacy4 { little_endian: bool },
    Legacy8 { little_endian: bool },
    Modern,
}

impl Layout {
    const fn header_len(self) -> usize {
        match self {
            Self::Legacy4 { .. } => 20,
            Self::Legacy8 { .. } => 24,
            Self::Modern => 32,
        }
    }

    fn payload_len(self, header: &[u8]) -> Option<usize> {
        match self {
            Self::Legacy4 { little_endian } | Self::Legacy8 { little_endian } => {
                let bytes: [u8; 4] = header.get(4..8)?.try_into().ok()?;
                let size = if little_endian {
                    i32::from_le_bytes(bytes)
                } else {
                    i32::from_be_bytes(bytes)
                };
                usize::try_from(size).ok()
            }
            Self::Modern => {
                let bytes: [u8; 8] = header.get(16..24)?.try_into().ok()?;
                usize::try_from(i64::from_le_bytes(bytes)).ok()
            }
        }
    }
}

/// A strategy that preserves large Blender block payloads across document saves.
#[derive(Debug, Clone, Copy, Default)]
pub struct Blend;

impl Chunker for Blend {
    fn split(&self, content: &[u8]) -> Vec<Range<usize>> {
        // An unknown or damaged Blender layout stays byte-exact as one object: guessed boundaries
        // cannot earn reliable sharing across saves. This is the policy's answer for data it cannot
        // account for rather than a bypass of anything, so it stays as long as the policy accepts
        // files it cannot parse.
        walk(content).unwrap_or_else(|| Whole.split(content))
    }
}

impl Policy for Blend {
    const NAME: &'static str = "blend-basic";
    const EXTENSIONS: &'static [&'static str] = &["blend", "blend1", "blend2"];
}

/// Read the file header only when its block layout is known.
fn layout(content: &[u8]) -> Option<(usize, Layout)> {
    if content.starts_with(b"BLENDER17-01v") {
        let version = content.get(13..17)?;
        if version.iter().all(u8::is_ascii_digit) {
            return Some((17, Layout::Modern));
        }
        return None;
    }

    if content.get(..7)? != b"BLENDER" || !content.get(9..12)?.iter().all(u8::is_ascii_digit) {
        return None;
    }

    let little_endian = match content[8] {
        b'v' => true,
        b'V' => false,
        _ => return None,
    };
    let layout = match content[7] {
        b'_' => Layout::Legacy4 { little_endian },
        b'-' => Layout::Legacy8 { little_endian },
        _ => return None,
    };
    Some((12, layout))
}

/// Return only boundaries supported by a complete Blender block stream.
fn walk(content: &[u8]) -> Option<Vec<Range<usize>>> {
    let (mut at, layout) = layout(content)?;
    let mut spans = Vec::new();
    let mut saw_dna = false;

    loop {
        let payload_at = at.checked_add(layout.header_len())?;
        let header = content.get(at..payload_at)?;
        let size = layout.payload_len(header)?;
        let end = payload_at.checked_add(size)?;
        let payload = content.get(payload_at..end)?;

        if header.get(..4)? == b"ENDB" {
            return (saw_dna && size == 0 && end == content.len())
                .then(|| crate::spans::covering(content.len(), spans));
        }
        if header.get(..4)? == b"DNA1" {
            if !payload.starts_with(b"SDNA") {
                return None;
            }
            saw_dna = true;
        }
        if size >= MIN_PAYLOAD {
            spans.push((payload_at, end));
        }
        at = end;
    }
}

#[cfg(test)]
mod tests {
    use super::{Blend, Chunker as _, Whole};

    fn block(code: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::from(code);
        bytes.extend_from_slice(&0_i32.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&i64::try_from(payload.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&1_i64.to_le_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    fn document(first: &[u8]) -> Vec<u8> {
        let mut bytes = b"BLENDER17-01v0502".to_vec();
        bytes.extend(block(*b"OB\0\0", first));
        bytes.extend(block(*b"DATA", &[7; 64]));
        bytes.extend(block(*b"DNA1", &[b"SDNA".as_slice(), &[3; 4096]].concat()));
        bytes.extend(block(*b"ENDB", &[]));
        bytes
    }

    fn legacy_block(code: [u8; 4], payload: &[u8], pointer_size: usize, little: bool) -> Vec<u8> {
        let mut bytes = Vec::from(code);
        let length = i32::try_from(payload.len()).unwrap();
        let length_bytes = if little {
            length.to_le_bytes()
        } else {
            length.to_be_bytes()
        };
        bytes.extend_from_slice(&length_bytes);
        bytes.resize(8 + pointer_size, 0);
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(payload);
        bytes
    }

    fn legacy_document(pointer_size: usize, little: bool) -> Vec<u8> {
        let mut bytes = b"BLENDER".to_vec();
        bytes.push(if pointer_size == 4 { b'_' } else { b'-' });
        bytes.push(if little { b'v' } else { b'V' });
        bytes.extend_from_slice(b"405");
        bytes.extend(legacy_block(*b"OB\0\0", &[5; 4096], pointer_size, little));
        bytes.extend(legacy_block(
            *b"DNA1",
            &[b"SDNA".as_slice(), &[3; 4096]].concat(),
            pointer_size,
            little,
        ));
        bytes.extend(legacy_block(*b"ENDB", &[], pointer_size, little));
        bytes
    }

    #[test]
    fn large_payloads_are_whole_and_every_byte_is_covered() {
        let content = document(&[5; 4096]);
        let ranges = Blend.split(&content);
        let joined: Vec<u8> = ranges
            .iter()
            .flat_map(|range| &content[range.clone()])
            .copied()
            .collect();

        assert_eq!(joined, content);
        assert!(ranges.contains(&(49..4145)));
        assert_eq!(ranges, Blend.split(&content));
    }

    #[test]
    fn a_changed_small_block_keeps_a_large_payload() {
        let before = document(&[5; 4096]);
        let mut after = before.clone();
        after[4145 + 32] = 8;

        let chunk = &before[49..4145];
        assert!(
            Blend
                .split(&after)
                .iter()
                .any(|range| &after[range.clone()] == chunk)
        );
    }

    #[test]
    fn legacy_pointer_sizes_and_byte_orders_are_read() {
        for pointer_size in [4, 8] {
            for little in [false, true] {
                let content = legacy_document(pointer_size, little);
                let ranges = Blend.split(&content);
                let payload_at = 12 + if pointer_size == 4 { 20 } else { 24 };
                let joined: Vec<u8> = ranges
                    .iter()
                    .flat_map(|range| &content[range.clone()])
                    .copied()
                    .collect();

                assert_eq!(joined, content);
                assert!(ranges.contains(&(payload_at..payload_at + 4096)));
            }
        }
    }

    #[test]
    fn damaged_or_unknown_layouts_stay_whole() {
        let mut damaged = document(&[5; 4096]);
        damaged[33..41].copy_from_slice(&i64::MAX.to_le_bytes());

        let mut trailing = document(&[5; 4096]);
        trailing.push(0);

        assert_eq!(Blend.split(&damaged), Whole.split(&damaged));
        assert_eq!(Blend.split(&trailing), Whole.split(&trailing));
        assert_eq!(
            Blend.split(b"not a Blender file"),
            Whole.split(b"not a Blender file")
        );
    }
}
