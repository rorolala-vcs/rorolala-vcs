//! Cutting the fragment a hit is shown with out of the field the query matched.
//!
//! A fragment is a window around the words that matched, cut on character boundaries so that a
//! range handed back always splits the text cleanly. When the window is shorter than the field it
//! came from, the cut ends are marked with `…`, and the ranges are counted from the marked text so
//! a caller never has to know where the marks were put.

use std::collections::BTreeSet;

use crate::article::{Article, Field, Range, Snippet};
use crate::token;

/// How many fragments one hit is shown with at most.
///
/// Three is what a list of hits has room for: a hit that matched in more places than that is
/// usually a short article or a very common word, and the fourth fragment would say nothing the
/// first three did not.
pub const MAX_SNIPPETS_PER_HIT: usize = 3;

/// How many characters before a match the window starts, when the field is longer.
const CONTEXT_BEFORE: usize = 48;

/// How many characters after a match the window reaches.
const CONTEXT_AFTER: usize = 96;

/// How close two matches have to be to be shown as one fragment.
const WINDOW_GAP: usize = 24;

/// The fragments of `article` that `query` matched, the field that reads best first.
///
/// The search asks for every field of the article, so a query that names only a title still has a
/// fragment to show: the text is cut from the field the word was actually in. Content comes first
/// because a fragment of body text is what a hit is usually shown with; the other fields follow so
/// that a metadata-only match is not a hit with nothing beside it.
pub fn snippets(article: &Article, query: &[String]) -> Vec<Snippet> {
    let wanted: BTreeSet<&str> = query.iter().map(String::as_str).collect();

    for field in Field::SNIPPET_ORDER {
        let text = article.text(field);
        let found = matched(&text, &wanted);

        if found.is_empty() {
            continue;
        }

        let length = text.chars().count();
        let snippets: Vec<Snippet> = windows(length, &found)
            .into_iter()
            .map(|(start, end)| cut(&text, field, start, end, &found))
            .collect();

        if !snippets.is_empty() {
            return snippets;
        }
    }

    Vec::new()
}

/// The ranges of `text` whose terms are in `wanted`, without the ones another range already covers.
///
/// A Chinese query is cut into characters and pairs, so a phrase yield ranges that overlap: the
/// pair covers the two characters, and each character covers one. The pair is the range worth
/// showing, since it is the one that says where the phrase is, so a range that lies inside another
/// is dropped rather than drawn twice.
fn matched(text: &str, wanted: &BTreeSet<&str>) -> Vec<Range> {
    let mut found: Vec<Range> = token::terms(text)
        .into_iter()
        .filter(|term| wanted.contains(term.text.as_str()))
        .map(|term| Range {
            start: term.start,
            end: term.end,
        })
        .collect();

    found.sort_by(|left, right| {
        left.start
            .cmp(&right.start)
            .then_with(|| right.end.cmp(&left.end))
    });

    let mut kept = Vec::new();
    let mut furthest = 0;
    for range in found {
        if range.length() == 0 || range.end <= furthest {
            continue;
        }

        furthest = range.end;
        kept.push(range);
    }

    kept
}

/// The windows to cut, each around one match, with the ones that are close enough merged.
fn windows(length: usize, found: &[Range]) -> Vec<(usize, usize)> {
    let mut windows: Vec<(usize, usize)> = Vec::new();

    for range in found {
        let start = range.start.saturating_sub(CONTEXT_BEFORE);
        let end = range.end.saturating_add(CONTEXT_AFTER).min(length);

        if let Some(last) = windows.last_mut()
            && start <= last.1.saturating_add(WINDOW_GAP)
        {
            last.1 = last.1.max(end);
            continue;
        }

        if windows.len() >= MAX_SNIPPETS_PER_HIT {
            break;
        }

        windows.push((start, end));
    }

    windows
}

/// Cuts one fragment out of `text`, marking the cut ends and moving the ranges onto what is left.
fn cut(text: &str, field: Field, start: usize, end: usize, found: &[Range]) -> Snippet {
    let mut start = start;
    let mut end = end;

    while start < end && character(text, start).is_some_and(char::is_whitespace) {
        start += 1;
    }
    while end > start && character(text, end - 1).is_some_and(char::is_whitespace) {
        end -= 1;
    }

    let trimmed_front = start > 0;
    let trimmed_back = end < text.chars().count();
    let mut body = String::new();

    if trimmed_front {
        body.push('…');
    }

    let shift = usize::from(trimmed_front);
    body.extend(characters(text, start, end));

    if trimmed_back {
        body.push('…');
    }

    let mut ranges: Vec<Range> = Vec::new();
    for range in found
        .iter()
        .filter(|range| range.start >= start && range.end <= end)
        .map(|range| Range {
            start: range.start - start + shift,
            end: range.end - start + shift,
        })
    {
        if let Some(last) = ranges.last_mut()
            && range.start <= last.end
        {
            last.end = last.end.max(range.end);
            continue;
        }

        ranges.push(range);
    }

    Snippet {
        field,
        text: body,
        ranges,
    }
}

/// The character at `at`, if there is one.
fn character(text: &str, at: usize) -> Option<char> {
    text.chars().nth(at)
}

/// The characters from `start` to `end`, counted in characters.
fn characters(text: &str, start: usize, end: usize) -> impl Iterator<Item = char> + '_ {
    text.chars().skip(start).take(end.saturating_sub(start))
}

#[cfg(test)]
mod tests {
    use crate::article::{Article, Field, Range};

    use super::snippets;

    fn article() -> Article {
        Article::new(
            "install".to_owned(),
            "Install Rorolala".to_owned(),
            "Put the program on your machine".to_owned(),
            vec!["install".to_owned()],
            String::new(),
            "The quick brown fox jumps over the lazy dog and then it installs the program \
             with the quick brown fox watching over it"
                .to_owned(),
        )
    }

    fn query(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn a_fragment_holds_the_word_and_the_ranges_point_at_it() {
        let snippets = snippets(&article(), &query(&["brown"]));

        assert_eq!(snippets.len(), 1);
        let snippet = &snippets[0];
        assert_eq!(snippet.field, Field::Content);
        assert!(snippet.text.contains("brown"));

        for range in &snippet.ranges {
            let got: String = snippet
                .text
                .chars()
                .skip(range.start)
                .take(range.length())
                .collect();

            assert_eq!(got, "brown");
        }
    }

    #[test]
    fn a_field_with_no_match_falls_back_to_the_one_that_has_it() {
        let snippets = snippets(&article(), &query(&["rorolala"]));

        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0].field, Field::Title);
        assert!(snippets[0].text.contains("Rorolala"));
    }

    #[test]
    fn a_word_that_matched_nowhere_has_no_fragment() {
        assert_eq!(snippets(&article(), &query(&["nonexistent"])).len(), 0);
    }

    #[test]
    fn a_cut_fragment_marks_the_ends_it_was_cut_from() {
        let longer = Article::new(
            "long".to_owned(),
            "Long".to_owned(),
            String::new(),
            Vec::new(),
            String::new(),
            format!("{} needle {}", "before ".repeat(40), "after ".repeat(40)),
        );

        let snippet = &snippets(&longer, &query(&["needle"]))[0];
        assert!(snippet.text.starts_with('…'));
        assert!(snippet.text.ends_with('…'));

        for range in &snippet.ranges {
            let got: String = snippet
                .text
                .chars()
                .skip(range.start)
                .take(range.length())
                .collect();

            assert_eq!(got, "needle");
        }
    }

    #[test]
    fn a_chinese_phrase_is_marked_by_its_pair_rather_than_each_character() {
        let article = Article::new(
            "cn".to_owned(),
            "Chinese".to_owned(),
            String::new(),
            Vec::new(),
            String::new(),
            "这是一段关于安装的文字".to_owned(),
        );

        let snippet = &snippets(&article, &query(&["安装"]))[0];
        assert!(
            snippet
                .ranges
                .iter()
                .map(Range::length)
                .all(|length| length == 2),
            "a phrase should be marked by the pair, not by each character: {:?}",
            snippet.ranges
        );
        assert!(
            snippet
                .ranges
                .iter()
                .all(|range| range.end <= range.start + 2),
            "overlapping pairs are joined rather than drawn twice: {:?}",
            snippet.ranges
        );
    }
}
