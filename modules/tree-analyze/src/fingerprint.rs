//! What a file looks like to the analysis: a digest of its content, and the spans it is cut into.
//!
//! Two files are the same when their digests are. When they are only *alike* — a file that was
//! edited after being moved, say — the digests differ, and the likeness is estimated the way git
//! estimates one: the content is cut into spans, the spans are counted, and what was copied from
//! one to the other is what is left once the counts are lined up. See git's `diffcore-delta.c`,
//! whose `hash_chars` and `diffcore_count_changes` this is a port of.

use std::collections::HashMap;

/// How much of a file is looked at to tell text from binary.
///
/// git's `FIRST_FEW_BYTES`: past this, a NUL is not looked for.
const FIRST_FEW_BYTES: usize = 8000;

/// How long a span may be, when a line does not cut it first.
const SPAN_LIMIT: u32 = 64;

/// What a span's hash is folded by, keeping the multiset of spans small.
const HASH_BASE: u32 = 107_927;

/// Whether `bytes` reads as binary: whether a NUL is in its opening.
#[must_use]
pub fn is_binary(bytes: &[u8]) -> bool {
    let opening = &bytes[..FIRST_FEW_BYTES.min(bytes.len())];

    opening.contains(&0)
}

/// The digest `bytes` is named by.
#[must_use]
pub fn digest(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

/// The spans `bytes` is cut into: each with its hash, and how many bytes it spans.
///
/// Content is cut at a line feed, or every [`SPAN_LIMIT`] bytes, whichever comes first; a text
/// file's carriage return before a line feed is dropped, so that the same text written with and
/// without CRLF reads the same. Equal spans are counted together, and what comes back is in hash
/// order, so two files' spans line up by walking both.
///
/// `binary` says whether the file read as binary; a binary file is cut the same way but keeps its
/// carriage returns, and never takes part in the likeness estimate that reads this.
#[must_use]
pub fn spans(bytes: &[u8], binary: bool) -> Vec<(u32, u32)> {
    let text = !binary;
    let mut counts: HashMap<u32, u32> = HashMap::new();

    let mut first: u32 = 0;
    let mut second: u32 = 0;
    let mut span: u32 = 0;
    let mut at = 0_usize;

    while at < bytes.len() {
        let byte = u32::from(bytes[at]);
        let previous = first;
        at += 1;

        if text && byte == 0x0d && at < bytes.len() && bytes[at] == 0x0a {
            continue;
        }

        first = (first << 7) ^ (second >> 25);
        second = (second << 7) ^ (previous >> 25);
        first = first.wrapping_add(byte);
        span += 1;

        if span < SPAN_LIMIT && byte != 0x0a {
            continue;
        }

        let hash = fold(first, second);
        *counts.entry(hash).or_insert(0) += span;
        span = 0;
        first = 0;
        second = 0;
    }

    if span > 0 {
        let hash = fold(first, second);
        *counts.entry(hash).or_insert(0) += span;
    }

    let mut spans: Vec<(u32, u32)> = counts.into_iter().collect();
    spans.sort_unstable();

    spans
}

/// The hash a span is counted under.
const fn fold(first: u32, second: u32) -> u32 {
    first.wrapping_add(second.wrapping_mul(0x61)) % HASH_BASE
}

/// How many bytes of `destination` are copied from `source`.
///
/// Both are the span lists [`spans`] gives, in hash order, so lining them up is a walk: a span the
/// two share is copied as far as the smaller count reaches, and a span only one has is not copied
/// at all. The number is what the likeness is worked out of — see
/// [`TreeDiff`](crate::TreeDiff).
#[must_use]
pub fn copied(source: &[(u32, u32)], destination: &[(u32, u32)]) -> u64 {
    let mut copied = 0_u64;
    let mut at = 0_usize;

    for &(hash, count) in source {
        while at < destination.len() && destination[at].0 < hash {
            at += 1;
        }

        let shared = if at < destination.len() && destination[at].0 == hash {
            let count = destination[at].1;
            at += 1;
            count
        } else {
            0
        };

        copied += u64::from(count.min(shared));
    }

    copied
}

#[cfg(test)]
mod tests {
    use super::{copied, digest, is_binary, spans};

    #[test]
    fn a_nul_in_the_opening_is_what_makes_a_file_binary() {
        assert!(!is_binary(b"plain text\n"));
        assert!(is_binary(b"a\0b"));

        // A NUL past the opening is not looked for: what git calls binary is what it sniffs.
        let mut late = vec![b'a'; 9000];
        late.push(0);
        assert!(!is_binary(&late));
    }

    #[test]
    fn the_same_content_has_the_same_digest() {
        assert_eq!(digest(b"rorolala"), digest(b"rorolala"));
        assert_ne!(digest(b"rorolala"), digest(b"rorolala!"));
    }

    #[test]
    fn an_unchanged_file_is_all_copied_and_a_changed_one_is_not() {
        let one = spans(b"line one\nline two\nline three\n", false);
        let same = spans(b"line one\nline two\nline three\n", false);
        let other = spans(b"line one\nline two\nline three changed\n", false);

        assert_eq!(copied(&one, &same), 29);
        assert!(copied(&one, &other) < 29);
        assert!(copied(&one, &other) > 0);
    }

    #[test]
    fn carriage_returns_do_not_change_how_text_reads() {
        // The same text written with and without CRLF is the same text.
        assert_eq!(spans(b"a\nb\n", false), spans(b"a\r\nb\r\n", false),);
        // A binary file keeps them, so the two are not forced together there.
        assert_ne!(spans(b"a\nb\n", true), spans(b"a\r\nb\r\n", true));
    }
}
