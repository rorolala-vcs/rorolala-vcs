//! Cutting a file into the spans that are worth keeping apart from the rest.
//!
//! A format-aware cut is nearly always the same shape: find the spans of the container that are the
//! container's *contents* — a ZIP's member, a PNG's chunk payload — and cut those out whole, letting
//! everything between them be ranges of its own. That is what makes an edit to one member leave every
//! member it did not touch a chunk the store already has.
//!
//! The between-bytes are why this is one function rather than a line in each chunker: a container
//! holds headers, descriptors and trailers that belong to no member, and a cut that dropped them
//! would not put its input back together. What belongs to nothing is a range of its own, so what comes
//! back out is always all of what went in.

use std::ops::Range;

/// The spans turned into ranges that cover `len` exactly and in order.
///
/// A span is cut out whole where it is; what lies before it, between it and the next, and after the
/// last is a range of its own. Spans that overlap, or that reach past `len`, are pulled back into the
/// one before them rather than refused: a container a writer built loosely is still a container, and
/// the answer that always holds is the one where every byte is covered once.
pub fn covering(len: usize, spans: Vec<(usize, usize)>) -> Vec<Range<usize>> {
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

    // Whatever is left over belongs to no span, and it is cut as one.
    if at < len {
        ranges.push(at..len);
    }

    ranges
}

#[cfg(test)]
mod tests {
    use super::covering;

    #[test]
    fn what_belongs_to_no_span_is_a_range_of_its_own() {
        // A header before, a gap between, and a trailer after: every byte covered once, in order.
        let ranges = covering(20, vec![(4, 8), (12, 16)]);

        assert_eq!(ranges, [0..4, 4..8, 8..12, 12..16, 16..20]);
    }

    #[test]
    fn spans_that_overlap_or_run_past_the_end_are_pulled_back() {
        // A container a writer built loosely still covers its bytes once.
        let ranges = covering(10, vec![(2, 6), (4, 20)]);

        assert_eq!(ranges, [0..2, 2..6, 6..10]);
    }

    #[test]
    fn nothing_but_spans_is_still_all_of_it() {
        assert_eq!(covering(0, Vec::new()), Vec::new());

        let all = covering(3, Vec::new());
        assert_eq!(all, covering(3, vec![(0, 3)]));
        assert_eq!(all.len(), 1);
        assert_eq!(all.first(), Some(&(0..3)));
    }
}
