//! How text is cut into the terms a query names.
//!
//! A term is what the index is keyed by, so the two sides of a search — the book written by a
//! build tool and the query typed by a reader — have to be cut the same way. That is why this is
//! one function rather than two.
//!
//! Text that separates its words with spaces is cut into words. Text that does not, which is most
//! Chinese and Japanese prose, is cut into every character on its own and every pair of adjacent
//! characters. The characters let a one-character query find its line; the pairs let a longer
//! query find its phrase, because a sentence holding all of them in a row has them all in a row.
//! A Latin word and a Chinese phrase in one query are cut by the same rule on either side of the
//! boundary between them, and the search asks for all of the terms, so `rola 安装` finds the line
//! that has both.

use unicode_normalization::UnicodeNormalization as _;

/// One term, and the characters of the text it was cut from.
///
/// The position is the text's own, in characters, and survives normalization: a term is
/// normalized for comparison, and what it points at is still where it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    /// The term itself: lowercased and compatibility-normalized, and so not necessarily what the
    /// text spells.
    pub text: String,
    /// The first character of the text the term covers.
    pub start: usize,
    /// One past the last character of the text the term covers.
    pub end: usize,
}

/// Cuts `text` into the terms a search names it by.
///
/// Runs of letters and digits are words; each Chinese-style character is a term of its own and
/// each pair beside it another. Everything else — spaces, punctuation, symbols — only ends a run,
/// and is otherwise ignored.
#[must_use]
pub fn terms(text: &str) -> Vec<Term> {
    let characters: Vec<char> = text.chars().collect();
    let mut terms = Vec::new();
    let mut at = 0;

    while at < characters.len() {
        let character = characters[at];

        if is_cjk(character) {
            let start = at;
            while at < characters.len() && is_cjk(characters[at]) {
                at += 1;
            }
            cut_cjk(&characters[start..at], start, &mut terms);
        } else if character.is_alphanumeric() {
            let start = at;
            while at < characters.len()
                && !is_cjk(characters[at])
                && characters[at].is_alphanumeric()
            {
                at += 1;
            }
            let word: String = characters[start..at].iter().collect();

            terms.push(Term {
                text: normalized(&word),
                start,
                end: at,
            });
        } else {
            at += 1;
        }
    }

    terms
}

/// Cuts a run of Chinese-style characters into single characters and adjacent pairs.
fn cut_cjk(run: &[char], offset: usize, terms: &mut Vec<Term>) {
    for (index, character) in run.iter().enumerate() {
        terms.push(Term {
            text: normalized(&character.to_string()),
            start: offset + index,
            end: offset + index + 1,
        });
    }

    for (index, pair) in run.windows(2).enumerate() {
        let text: String = pair.iter().collect();

        terms.push(Term {
            text: normalized(&text),
            start: offset + index,
            end: offset + index + 2,
        });
    }
}

/// Whether the character is one of the scripts that are written without spaces between words.
///
/// The ranges are the CJK Unified Ideographs and their extensions, the compatibility ideographs,
/// kana, hangul and bopomofo. Treating them as one family is deliberate: a document that mixes
/// them is cut into characters and pairs all the same, and no pair is formed across a space.
const fn is_cjk(character: char) -> bool {
    matches!(
        character as u32,
        0x1100..=0x11FF
            | 0x2E80..=0x2EFF
            | 0x3040..=0x30FF
            | 0x3100..=0x312F
            | 0x31A0..=0x31BF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xAC00..=0xD7AF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2FA1F
    )
}

/// The string a term is compared by: lowercased, then compatibility-normalized, so that a word
/// typed with a full-width letter and one typed with an ASCII letter are the same term.
fn normalized(text: &str) -> String {
    text.to_lowercase().nfkc().collect()
}

/// Whether the term is a word of a script written with spaces: letters and digits, no CJK.
///
/// Only such a word has fragments built for it: Chinese-style text is already cut into characters
/// and pairs, which is the same job for a script that does not separate words.
#[must_use]
pub fn is_latin(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|character| character.is_alphanumeric() && !is_cjk(character))
}

/// The character n-grams of a word: every run of two and of three characters.
///
/// These are what a query is looked up by when it is not a whole word — a prefix or a substring of
/// a word holds all of its own n-grams, so an article whose word holds all of them is a candidate.
/// Two characters is the shortest query a fragment is made for, which is what makes `in` a prefix
/// of `install` rather than a query with nothing to look it up by.
#[must_use]
pub fn shingles(word: &str) -> Vec<String> {
    let characters: Vec<char> = word.chars().collect();
    let mut shingles = Vec::new();

    for size in [2, 3] {
        for window in characters.windows(size) {
            let text: String = window.iter().collect();
            shingles.push(normalized(&text));
        }
    }

    shingles
}

#[cfg(test)]
mod tests {
    use super::{Term, is_latin, shingles, terms};

    fn texts(text: &str) -> Vec<String> {
        terms(text).into_iter().map(|term| term.text).collect()
    }

    #[test]
    fn a_latin_sentence_is_cut_into_words() {
        assert_eq!(
            texts("A PSD file, 2 times."),
            ["a", "psd", "file", "2", "times"]
        );
    }

    #[test]
    fn a_chinese_phrase_is_cut_into_characters_and_pairs() {
        assert_eq!(texts("安装"), ["安", "装", "安装"]);
        assert_eq!(
            texts("版本控制"),
            ["版", "本", "控", "制", "版本", "本控", "控制"]
        );
    }

    #[test]
    fn a_cjk_and_latin_mix_is_cut_on_the_boundary() {
        assert_eq!(texts("PSD文件"), ["psd", "文", "件", "文件"]);
    }

    #[test]
    fn terms_keep_the_positions_they_were_cut_from() {
        let terms = terms("值a");

        assert_eq!(
            terms
                .iter()
                .map(|term| (term.start, term.end))
                .collect::<Vec<_>>(),
            [(0, 1), (1, 2)]
        );
    }

    #[test]
    fn case_and_width_are_the_same_term() {
        assert_eq!(texts("ＲＯＬＡ"), ["rola"]);
        assert_eq!(texts("Rola"), ["rola"]);
    }

    #[test]
    fn punctuation_ends_a_run_and_is_otherwise_ignored() {
        assert_eq!(texts("a—b"), ["a", "b"]);
        assert_eq!(texts("---"), Vec::<String>::new());
    }

    #[test]
    fn a_term_is_where_it_was_even_when_normalization_changes_it() {
        let cut: Vec<Term> = terms("ﬁle");

        assert_eq!(cut.len(), 1);
        assert_eq!(cut[0].text, "file");
        assert_eq!((cut[0].start, cut[0].end), (0, 3));
    }

    #[test]
    fn only_a_word_of_a_spaced_script_is_latin() {
        assert!(is_latin("install"));
        assert!(is_latin("psd2"));
        assert!(!is_latin("安装"));
        assert!(!is_latin("install安装"));
        assert!(!is_latin(""));
    }

    #[test]
    fn a_word_is_cut_into_two_and_three_character_fragments() {
        let fragments = shingles("install");

        assert!(fragments.iter().any(|fragment| fragment == "in"));
        assert!(fragments.iter().any(|fragment| fragment == "ins"));
        assert!(!fragments.iter().any(|fragment| fragment == "i"));
        assert!(!fragments.iter().any(|fragment| fragment == "nstall"));
        assert_eq!(shingles("in"), ["in"]);
        assert_eq!(shingles("a").len(), 0);
    }

    #[test]
    fn a_fragment_is_normalized_like_a_term() {
        assert_eq!(shingles("ＲＯ"), ["ro"]);
    }
}
