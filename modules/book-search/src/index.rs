//! The book a program searches: where it is opened from, and how a query is answered.
//!
//! The whole book is held in memory once it is read, which is what makes a query a walk of the
//! index rather than a read of the disk. A book is documentation: some hundred articles and a few
//! megabytes, read once when a program starts searching and asked many times after that.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use rorolala_utils_lazyffi::lazyffi;

use crate::format::{self, Decoded, FIELD_COUNT};
use crate::{Article, BOOK_RELATIVE_PATH, BookError, Field, Hit, token};

/// The BM25 term that says how fast repeated uses of one term stop adding to a score.
const K1: f64 = 1.2;

/// The BM25 term that says how much a longer field is worth less than a short one.
const B: f64 = 0.75;

/// How much a match found through a word's fragments is worth, against the same word found whole.
///
/// A fragment match is divided by how many fragments made it, so a long word does not out-score a
/// whole-word match merely by having more of them, and the whole is scaled below a whole word.
const FUZZY_WEIGHT: f64 = 0.25;

/// The shortest Latin query that is looked up through the fragment index.
const SHORTEST_FRAGMENT: usize = 2;

/// Whether `term` may be looked up through the fragment index.
///
/// Only a Latin word of at least two characters is: a Chinese-style term is already a character or
/// a pair, and a one-character Latin query has no fragment to be found by.
fn has_fragments(term: &str) -> bool {
    token::is_latin(term) && term.chars().count() >= SHORTEST_FRAGMENT
}

/// How many directories above the one it is given [`BookSearch::locate`] looks.
///
/// The programs are laid in `bin/` and the Desktop program one directory deeper, so the book is
/// two or three directories above either of them. Four is one more than that, which leaves room
/// for a layout that puts one more directory between the two.
const LOCATE_ANCESTORS: usize = 5;

/// The searchable content of a book.
///
/// Opening a book reads it whole and checks it against the digest it carries; a book that does not
/// read is refused rather than searched. Once open, [`Self::search`] answers a query without
/// touching the disk again.
#[lazyffi(export = RolaBookSearch)]
#[derive(Debug)]
pub struct BookSearch {
    /// Every article, in the order the file names them.
    articles: Vec<Article>,
    /// Where an article's name finds its position in [`Self::articles`].
    by_name: BTreeMap<String, usize>,
    /// How many terms each article has in each field, for the relevance ranking.
    lengths: Vec<[u32; FIELD_COUNT]>,
    /// The average length of a field across the book.
    averages: [f64; FIELD_COUNT],
    /// The postings of each term, by field.
    terms: BTreeMap<String, crate::format::TermFields>,
    /// The postings of each fragment of a Latin word, by field.
    fragments: BTreeMap<String, crate::format::TermFields>,
    /// How many articles there are, in the number the ranking is worked out in.
    documents: f64,
}

impl BookSearch {
    /// Opens the book at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`BookError`] when the file cannot be read, when it is not a book this build knows,
    /// or when it is a format version this build does not read.
    pub fn open(path: &Path) -> Result<Self, BookError> {
        let bytes = std::fs::read(path)?;

        Self::from_bytes(&bytes)
    }

    /// Opens the book the bytes of one file hold.
    ///
    /// # Errors
    ///
    /// As [`Self::open`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BookError> {
        Ok(Self::from_decoded(format::decode(bytes)?))
    }

    /// The book laid beside the programs under `start`, or nothing when there is none.
    ///
    /// The walk goes up from `start`, which is the directory a program is in, and looks for the
    /// book where an export or an install lays it: `docs/builtin.book` under the root that holds
    /// both the programs and the documentation. A directory on the way with no book in it is
    /// passed over, so the same call works for the command line program two directories under the
    /// root and for the Desktop program three.
    ///
    /// A file that is there but not a book this build can read is passed over too, and the walk
    /// goes on: what a caller asked for is a book to search, and a file that cannot be searched is
    /// not one.
    #[must_use]
    pub fn locate(start: &Path) -> Option<Self> {
        for root in start.ancestors().take(LOCATE_ANCESTORS) {
            let candidate = root.join(BOOK_RELATIVE_PATH);

            if candidate.is_file()
                && let Ok(book) = Self::open(&candidate)
            {
                return Some(book);
            }
        }

        None
    }

    /// How many articles the book holds.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.articles.len()
    }

    /// The article named `name`, if the book holds one.
    #[must_use]
    pub fn article(&self, name: &str) -> Option<&Article> {
        self.by_name
            .get(name)
            .and_then(|index| self.articles.get(*index))
    }

    /// The articles `query` finds, best first, at most `limit` of them.
    ///
    /// The query is cut into terms the same way the book was, and an article is a hit only when
    /// every one of them is in it: `rola 安装` asks for both, and `版本控制` asks for the phrase
    /// rather than the four characters anywhere. A Latin term is also looked up by the fragments
    /// its word was built with, so a query that is a prefix or a substring of a word — `inst` of
    /// `install`, `stall` of `install` — finds the word too; a whole-word match ranks above one
    /// found that way. A query with no terms in it — one that is all punctuation, or empty — finds
    /// nothing. `limit` of zero finds nothing too, which is what a caller asking for no lines means.
    ///
    /// Each hit carries the fragments the query matched in it, taken from the first field that has
    /// a match: the body if it is the body that named the word, and the description or title when
    /// it is only those. The fragments are cut from the text the book carries, and their ranges
    /// count characters of the fragment, so a caller can style the matched pieces without parsing
    /// the text again.
    #[must_use]
    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        if limit == 0 {
            return Vec::new();
        }

        let mut seen: BTreeSet<String> = BTreeSet::new();
        let query_terms: Vec<String> = token::terms(query)
            .into_iter()
            .map(|term| term.text)
            .filter(|text| seen.insert(text.clone()))
            .collect();

        if query_terms.is_empty() {
            return Vec::new();
        }

        let Some(candidates) = self.candidates(&query_terms) else {
            return Vec::new();
        };

        let mut scored: Vec<(f64, usize)> = candidates
            .into_iter()
            .map(|article| (self.score(article, &query_terms), article))
            .collect();

        scored.sort_by(|left, right| {
            right
                .0
                .total_cmp(&left.0)
                .then_with(|| self.name_of(left.1).cmp(self.name_of(right.1)))
        });

        scored
            .into_iter()
            .take(limit)
            .filter_map(|(score, article)| self.hit(score, article, &query_terms))
            .collect()
    }

    /// Works the book out from what the file said.
    fn from_decoded(decoded: Decoded) -> Self {
        let Decoded {
            articles,
            terms,
            fragments,
        } = decoded;
        let mut lengths = vec![[0_u32; FIELD_COUNT]; articles.len()];
        let mut totals = [0.0_f64; FIELD_COUNT];

        for fields in terms.values() {
            for field in Field::ALL {
                for posting in &fields[field.index()] {
                    let Some(lengths) = lengths.get_mut(posting.article) else {
                        continue;
                    };
                    let Some(length) = lengths.get_mut(field.index()) else {
                        continue;
                    };

                    *length = length.saturating_add(posting.tf);
                    totals[field.index()] += f64::from(posting.tf);
                }
            }
        }

        let documents = f64::from(u32::try_from(articles.len()).unwrap_or(u32::MAX));
        let averages = totals.map(|total| total / documents);
        let by_name = articles
            .iter()
            .enumerate()
            .map(|(index, article)| (article.name().to_owned(), index))
            .collect();

        Self {
            articles,
            by_name,
            lengths,
            averages,
            terms,
            fragments,
            documents,
        }
    }

    /// The articles that hold every one of `query_terms`, if any do.
    fn candidates(&self, query_terms: &[String]) -> Option<BTreeSet<usize>> {
        let mut candidates: Option<BTreeSet<usize>> = None;

        for term in query_terms {
            let found = self.posted(term);

            if found.is_empty() {
                return None;
            }

            candidates = Some(match candidates {
                None => found,
                Some(current) => current.intersection(&found).copied().collect(),
            });
        }

        candidates
    }

    /// The articles that hold `term`, whole or inside one of their words.
    fn posted(&self, term: &str) -> BTreeSet<usize> {
        let mut found = BTreeSet::new();

        if let Some(fields) = self.terms.get(term) {
            for postings in fields {
                for posting in postings {
                    found.insert(posting.article);
                }
            }
        }

        if has_fragments(term) {
            found.extend(self.fragment_candidates(term));
        }

        found
    }

    /// The articles whose words hold every fragment of `term`, and the word that holds them.
    fn fragment_candidates(&self, term: &str) -> BTreeSet<usize> {
        let fragments: BTreeSet<String> = token::shingles(term).into_iter().collect();
        let mut candidates: Option<BTreeSet<usize>> = None;

        for fragment in fragments {
            let Some(fields) = self.fragments.get(&fragment) else {
                return BTreeSet::new();
            };

            let mut found = BTreeSet::new();
            for postings in fields {
                for posting in postings {
                    found.insert(posting.article);
                }
            }

            candidates = Some(match candidates {
                None => found,
                Some(current) => current.intersection(&found).copied().collect(),
            });

            if candidates.as_ref().is_some_and(BTreeSet::is_empty) {
                return BTreeSet::new();
            }
        }

        candidates
            .unwrap_or_default()
            .into_iter()
            .filter(|article| self.article_holds(*article, term))
            .collect()
    }

    /// Whether any field of `article` has a word that holds `term`.
    ///
    /// The fragments say an article is a candidate; this says the query is really there, so a
    /// fragment that landed in one word and another fragment in the next is not answered as if
    /// the query were one word.
    fn article_holds(&self, article: usize, term: &str) -> bool {
        let Some(article) = self.articles.get(article) else {
            return false;
        };

        Field::ALL.into_iter().any(|field| {
            let text = article.text(field);

            token::terms(&text)
                .iter()
                .any(|token| token.text.contains(term))
        })
    }

    /// How well `article` matches `query_terms`: BM25 over every field, each field weighted.
    fn score(&self, article: usize, query_terms: &[String]) -> f64 {
        query_terms
            .iter()
            .map(|term| self.term_score(article, term))
            .sum()
    }

    /// How well `article` matches one term: whole if it holds the word, otherwise by its fragments.
    fn term_score(&self, article: usize, term: &str) -> f64 {
        let exact = self.exact_score(article, term);

        if exact > 0.0 {
            return exact;
        }

        if has_fragments(term) {
            return self.fragment_score(article, term);
        }

        0.0
    }

    /// The BM25 score of `article` for `term` found whole, or zero when it is not there.
    fn exact_score(&self, article: usize, term: &str) -> f64 {
        let Some(lengths) = self.lengths.get(article) else {
            return 0.0;
        };
        let Some(fields) = self.terms.get(term) else {
            return 0.0;
        };

        let mut score = 0.0;

        for field in Field::ALL {
            let postings = &fields[field.index()];
            let Ok(at) = postings.binary_search_by_key(&article, |posting| posting.article) else {
                continue;
            };
            let Some(posting) = postings.get(at) else {
                continue;
            };

            let Some(average) = self.averages.get(field.index()).copied() else {
                continue;
            };
            if average <= 0.0 {
                continue;
            }

            let documents_found = f64::from(u32::try_from(postings.len()).unwrap_or(u32::MAX));
            let idf = ((self.documents - documents_found + 0.5) / (documents_found + 0.5)).ln_1p();
            let length = f64::from(lengths.get(field.index()).copied().unwrap_or(0));
            let frequency = f64::from(posting.tf);
            let denominator = K1.mul_add(1.0 - B + B * length / average, frequency);

            score += field.weight() * idf * frequency * (K1 + 1.0) / denominator;
        }

        score
    }

    /// The score of `article` for `term` found through the fragments of a word that holds it.
    ///
    /// The fragments are scored by how often they appear and how few articles have them, weighted
    /// by the field they were found in, and the whole is scaled below a whole-word match: a word a
    /// reader actually typed ranks above one a prefix happened to reach.
    fn fragment_score(&self, article: usize, term: &str) -> f64 {
        let fragments: BTreeSet<String> = token::shingles(term).into_iter().collect();
        let count = u32::try_from(fragments.len()).unwrap_or(u32::MAX);
        let mut score = 0.0;

        for fragment in fragments {
            let Some(fields) = self.fragments.get(&fragment) else {
                continue;
            };

            for field in Field::ALL {
                let postings = &fields[field.index()];
                let Ok(at) = postings.binary_search_by_key(&article, |posting| posting.article)
                else {
                    continue;
                };
                let Some(posting) = postings.get(at) else {
                    continue;
                };

                let documents_found = f64::from(u32::try_from(postings.len()).unwrap_or(u32::MAX));
                let idf =
                    ((self.documents - documents_found + 0.5) / (documents_found + 0.5)).ln_1p();

                score = (field.weight() * idf).mul_add(f64::from(posting.tf), score);
            }
        }

        score / f64::from(count.max(1)) * FUZZY_WEIGHT
    }

    /// One hit, with the fragments its article matched.
    fn hit(&self, score: f64, article: usize, query_terms: &[String]) -> Option<Hit> {
        let article = self.articles.get(article)?;

        Some(Hit {
            name: article.name().to_owned(),
            title: article.title().to_owned(),
            description: article.description().to_owned(),
            score,
            snippets: crate::snippet::snippets(article, query_terms),
        })
    }

    /// The name of the article at `article`, or an empty name past the end.
    fn name_of(&self, article: usize) -> &str {
        self.articles.get(article).map_or("", Article::name)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::BookSearch;
    use crate::{Article, BookError, SourceArticle, encode};

    fn article(name: &str, title: &str, description: &str, plain: &str) -> SourceArticle {
        SourceArticle {
            name: name.to_owned(),
            title: title.to_owned(),
            description: description.to_owned(),
            keywords: Vec::new(),
            markdown: plain.to_owned(),
            plain: plain.to_owned(),
        }
    }

    fn book() -> BookSearch {
        let articles = vec![
            article(
                "install",
                "Installing Rorolala",
                "How to install the program",
                "Install Rorolala from the release page and bind it to a vault",
            ),
            article(
                "workspace",
                "The Workspace",
                "What a workspace is",
                "A workspace is the local copy of the Layout you work in",
            ),
            article(
                "cn",
                "安装 Rorolala",
                "安装说明",
                "把程序下载下来，然后绑定一个仓库，就可以开始工作了",
            ),
        ];

        BookSearch::from_bytes(&encode(&articles).expect("the test book encodes"))
            .expect("the test book opens")
    }

    fn names(hits: &[crate::Hit]) -> Vec<&str> {
        hits.iter().map(|hit| hit.name.as_str()).collect()
    }

    #[test]
    fn a_name_finds_the_article_by_its_title_and_its_body() {
        let book = book();

        assert_eq!(names(&book.search("Rorolala", 10)), ["install", "cn"]);
        assert_eq!(names(&book.search("Workspace", 10)), ["workspace"]);
        assert_eq!(book.search("vault", 10)[0].name, "install");
    }

    #[test]
    fn a_chinese_query_finds_the_phrase_and_the_characters() {
        let book = book();

        assert_eq!(book.search("安装", 10)[0].name, "cn");
        assert_eq!(book.search("仓库", 10)[0].name, "cn");
        assert_eq!(book.search("库", 10)[0].name, "cn");
    }

    #[test]
    fn a_prefix_or_a_substring_finds_the_word_that_holds_it() {
        let book = book();

        assert_eq!(names(&book.search("instal", 10)), ["install"]);
        assert_eq!(names(&book.search("stall", 10)), ["install"]);
        assert_eq!(names(&book.search("nstall", 10)), ["install"]);
        assert_eq!(names(&book.search("INSTAL", 10)), ["install"]);
    }

    #[test]
    fn a_whole_word_ranks_above_a_word_that_only_holds_it() {
        let articles = vec![
            article("exact", "Exact", "", "Install it from the page"),
            article("inside", "Inside", "", "Installation instructions"),
        ];
        let book = BookSearch::from_bytes(&encode(&articles).expect("the test book encodes"))
            .expect("the test book opens");

        assert_eq!(names(&book.search("install", 10)), ["exact", "inside"]);
    }

    #[test]
    fn a_query_asks_for_every_term_it_names() {
        let book = book();

        // Both words are in `install`, and neither the bare `install` nor the bare `workspace`
        // names may find the article that holds only one of them.
        assert_eq!(
            names(&book.search("install workspace", 10)),
            [] as [&str; 0]
        );
        assert_eq!(names(&book.search("install release", 10)), ["install"]);
    }

    #[test]
    fn a_hit_carries_its_description_and_a_matched_fragment() {
        let book = book();
        let hit = &book.search("release", 10)[0];

        assert_eq!(hit.description, "How to install the program");
        assert_ne!(hit.snippets.len(), 0);
        assert!(hit.snippets[0].text.contains("release"));
    }

    #[test]
    fn the_limit_is_what_was_asked_for() {
        let book = book();

        assert_eq!(book.search("the", 1).len(), 1);
        assert_eq!(book.search("the", 0).len(), 0);
        assert!(book.search("the", 10).len() <= 2);
    }

    #[test]
    fn a_query_with_no_terms_finds_nothing() {
        assert_eq!(book().search("---", 10).len(), 0);
        assert_eq!(book().search("", 10).len(), 0);
    }

    #[test]
    fn a_title_match_outranks_a_body_match() {
        let book = book();

        // `Rorolala` is in `install`'s title and again in its body, and in `cn`'s title only.
        let hits = book.search("rorolala", 10);
        assert_eq!(hits.len(), 2);
        assert!(hits[0].score > hits[1].score);
    }

    #[test]
    fn an_article_is_found_by_its_name() {
        let book = book();

        assert_eq!(book.count(), 3);
        assert_eq!(
            book.article("install").map(Article::title),
            Some("Installing Rorolala")
        );
        assert!(book.article("missing").is_none());
    }

    #[test]
    fn a_book_is_located_from_the_programs_directory() {
        let root = std::env::temp_dir().join(format!("rola-book-locate-{}", std::process::id()));
        let docs = root.join("docs");
        std::fs::create_dir_all(&docs).expect("the fixture directory");
        std::fs::write(
            docs.join("builtin.book"),
            encode(&[article("a", "A", "", "a")]).expect("the fixture book"),
        )
        .expect("the fixture book file");

        let found = BookSearch::locate(&root.join("bin/desktop"));
        assert!(found.is_some());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_missing_book_is_not_located() {
        let at = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("__nonexistent-book-root");

        assert!(BookSearch::locate(&at).is_none());
    }

    #[test]
    fn a_file_that_is_not_a_book_is_refused() {
        assert!(matches!(
            BookSearch::from_bytes(b"not a book"),
            Err(BookError::Malformed { .. })
        ));
    }
}
