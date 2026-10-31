//! Cutting the fragment a hit is shown with out of the field the query matched.
//!
//! A fragment is a window around the words that matched, cut on character boundaries so that a
//! range handed back always splits the text cleanly. When the window is shorter than the field it
//! came from, the cut ends are marked with `…`, and the ranges are counted from the marked text so
//! a caller never has to know where the marks were put.

use crate::article::{Article, Field, Range, Snippet};
use crate::token;

/// How many fragments one hit is shown with at most.
///
/// One is what a list of hits has room for: the description beside it says what the article is,
/// and the first place the query matched says why it is in the list. A second fragment would take
/// the room the description needs without adding another reason to open the article.
pub const MAX_SNIPPETS_PER_HIT: usize = 1;

/// How many characters before a match the window starts, when the field is longer.
const CONTEXT_BEFORE: usize = 16;

/// How many characters after a match the window reaches.
const CONTEXT_AFTER: usize = 32;

/// How close two matches have to be to be shown as one fragment.
const WINDOW_GAP: usize = 8;

/// The fragments of `article` that `query` matched, the field that reads best first.
///
/// The search asks for every field of the article, so a query that names only a title still has a
/// fragment to show: the text is cut from the field the word was actually in. Content comes first
/// because a fragment of body text is what a hit is usually shown with; the other fields follow so
/// that a metadata-only match is not a hit with nothing beside it.
pub fn snippets(article: &Article, query: &[String]) -> Vec<Snippet> {
    for field in Field::SNIPPET_ORDER {
        let text = article.text(field);
        let found = matched(&text, query);

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

/// The ranges of `text` that `query` matched, without the ones another range already covers.
///
/// A Chinese query is cut into characters and pairs, so a phrase yields ranges that overlap: the
/// pair covers the two characters, and each character covers one. The pair is the range worth
/// showing, since it is the one that says where the phrase is, so a range that lies inside another
/// is dropped rather than drawn twice.
///
/// A Latin query is also matched inside a word: `inst` is marked where it stands in `install`, which
/// is the same place the fragment index found the word by.
fn matched(text: &str, query: &[String]) -> Vec<Range> {
    let mut found: Vec<Range> = Vec::new();

    for term in token::terms(text) {
        for wanted in query {
            if term.text == *wanted {
                found.push(Range {
                    start: term.start,
                    end: term.end,
                });
            } else if wanted.chars().count() >= 2
                && token::is_latin(wanted)
                && token::is_latin(&term.text)
            {
                found.extend(
                    inside(&term.text, wanted)
                        .into_iter()
                        .map(|(start, end)| Range {
                            start: term.start + start,
                            end: term.start + end,
                        }),
                );
            }
        }
    }

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

/// Every place `wanted` appears inside `word`, as character positions in the word.
fn inside(word: &str, wanted: &str) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut from = 0;

    while let Some(at) = word.get(from..).and_then(|rest| rest.find(wanted)) {
        let start_byte = from + at;
        let start = word
            .get(..start_byte)
            .map_or(0, |before| before.chars().count());
        let end = start + wanted.chars().count();

        found.push((start, end));
        from = start_byte + wanted.len();

        if from >= word.len() {
            break;
        }
    }

    found
}

/// The windows to cut, each around one match, with the ones that are close enough merged.
///
/// The count is settled before a merge is considered, so a fragment that is already the last one
/// is not stretched over a later match: with one fragment the window is the first match's own
/// context and nothing more.
fn windows(length: usize, found: &[Range]) -> Vec<(usize, usize)> {
    let mut windows: Vec<(usize, usize)> = Vec::new();

    for range in found {
        if windows.len() >= MAX_SNIPPETS_PER_HIT {
            break;
        }

        let start = range.start.saturating_sub(CONTEXT_BEFORE);
        let end = range.end.saturating_add(CONTEXT_AFTER).min(length);

        if let Some(last) = windows.last_mut()
            && start <= last.1.saturating_add(WINDOW_GAP)
        {
            last.1 = last.1.max(end);
            continue;
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
    fn a_prefix_is_marked_where_it_stands_in_its_word() {
        let snippets = snippets(&article(), &query(&["inst"]));

        assert_eq!(snippets.len(), 1);
        assert!(snippets[0].text.contains("inst"));

        for range in &snippets[0].ranges {
            let got: String = snippets[0]
                .text
                .chars()
                .skip(range.start)
                .take(range.length())
                .collect();

            assert_eq!(got, "inst");
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
