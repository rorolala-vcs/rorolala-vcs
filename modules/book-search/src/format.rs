//! The bytes a book is written as.
//!
//! One file holds both halves of a search: the articles with all their text, and the index the
//! text is found through. It is written whole and read whole, so there is no directory to keep in
//! step with the file and nothing beside it that a copy could leave behind.
//!
//! ```text
//! magic        b"ROLABOOK"             8
//! version      u32, big endian         4
//! digest       BLAKE3 of the body      32
//! body:
//!   articles   u32 count, then per article:
//!                name, title, description   length-prefixed UTF-8
//!                keywords                   u32 count, then length-prefixed UTF-8
//!                markdown, plain            length-prefixed UTF-8
//!   terms      u32 count, then per term, sorted:
//!                term                       length-prefixed UTF-8
//!                per field in `Field::ALL`  u32 count, then postings:
//!                                             article index u32, term count u32
//! ```
//!
//! Every count and every offset is big endian and the terms are written in sorted order, so the
//! same articles produce the same bytes on every machine: the file can be compared between two
//! builds, and a change in it is a change in the book rather than in the order a map happened to
//! walk.
//!
//! The digest is what makes the file safe to trust lazily: nothing past the header is read until
//! the body hashes to what the header says, so a file cut short or written over is a book that
//! refuses to open rather than a search over nonsense.

use std::collections::BTreeMap;

use crate::{BookError, Field, SourceArticle, token};

/// The eight bytes every book begins with.
pub const MAGIC: [u8; 8] = *b"ROLABOOK";

/// The format this build writes and the only one it reads.
pub const FORMAT_VERSION: u32 = 1;

/// The number of fields a term keeps postings for.
pub const FIELD_COUNT: usize = 5;

/// How many bytes the header is: the magic, the version and the digest.
pub const HEADER_LEN: usize = MAGIC.len() + 4 + 32;

/// Where the version sits inside the header.
const VERSION_AT: usize = MAGIC.len();

/// Where the digest sits inside the header.
const DIGEST_AT: usize = VERSION_AT + 4;

/// One article's postings for one term, in one field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Posting {
    /// The article's position in the book's article table.
    pub article: usize,
    /// How many times the term appears in that field of that article.
    pub tf: u32,
}

/// A term's postings for every field, in [`Field::ALL`]'s order.
pub type TermFields = [Vec<Posting>; FIELD_COUNT];

/// A term with an empty posting list for every field.
pub fn fields() -> TermFields {
    std::array::from_fn(|_| Vec::new())
}

/// A book that has been read back, before the averages a search needs have been worked out.
pub struct Decoded {
    /// The articles, in the order the file names them.
    pub articles: Vec<crate::Article>,
    /// The postings, keyed by term.
    pub terms: BTreeMap<String, TermFields>,
}

/// Writes `articles` as one book file.
///
/// The articles are written in name order whatever order they arrive in, and two of one name are
/// refused: a name is what a hit is opened by, so two articles cannot share one.
///
/// # Errors
///
/// Returns [`BookError::Empty`] when there is nothing to write, [`BookError::Duplicate`] when two
/// articles have one name, and [`BookError::Oversized`] when a count or a length is larger than the
/// format can name.
pub fn encode(articles: &[SourceArticle]) -> Result<Vec<u8>, BookError> {
    if articles.is_empty() {
        return Err(BookError::Empty);
    }

    let mut ordered: Vec<&SourceArticle> = articles.iter().collect();
    ordered.sort_by(|left, right| left.name.cmp(&right.name));

    for pair in ordered.windows(2) {
        if pair[0].name == pair[1].name {
            return Err(BookError::Duplicate {
                name: pair[0].name.clone(),
            });
        }
    }

    let terms = collect_terms(&ordered);
    let mut body = Encoder::default();

    body.u32(narrow(ordered.len(), "the number of articles")?);
    for article in &ordered {
        body.string(&article.name)?;
        body.string(&article.title)?;
        body.string(&article.description)?;
        body.u32(narrow(article.keywords.len(), "an article's keywords")?);
        for keyword in &article.keywords {
            body.string(keyword)?;
        }
        body.string(&article.markdown)?;
        body.string(&article.plain)?;
    }

    body.u32(narrow(terms.len(), "the number of terms")?);
    for (term, postings) in &terms {
        body.string(term)?;
        for field in Field::ALL {
            let postings = postings[field.index()].as_slice();
            body.u32(narrow(postings.len(), "a term's postings")?);
            for posting in postings {
                body.u32(narrow(posting.article, "an article's position")?);
                body.u32(posting.tf);
            }
        }
    }

    let body = body.finish();
    let mut file = Vec::with_capacity(HEADER_LEN + body.len());

    file.extend_from_slice(&MAGIC);
    file.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
    file.extend_from_slice(blake3::hash(&body).as_bytes());
    file.extend_from_slice(&body);

    Ok(file)
}

/// Reads the articles and the index out of one book file.
///
/// # Errors
///
/// Returns [`BookError::Malformed`] when the bytes are not a book this build can read — not the
/// format's name, a digest that does not come back, a count or a length that runs past the file, a
/// posting for an article the file does not hold — and [`BookError::Unsupported`] when the format
/// version is one this build does not write.
pub fn decode(bytes: &[u8]) -> Result<Decoded, BookError> {
    let header = bytes
        .get(..HEADER_LEN)
        .ok_or_else(|| malformed("the file is shorter than its header"))?;

    if !bytes.starts_with(&MAGIC) {
        return Err(malformed(
            "the file does not begin with the book format's name",
        ));
    }

    let version = u32::from_be_bytes(
        header[VERSION_AT..VERSION_AT + 4]
            .try_into()
            .map_err(|_| malformed("the header's version is not four bytes"))?,
    );

    if version != FORMAT_VERSION {
        return Err(BookError::Unsupported { version });
    }

    let body = bytes
        .get(HEADER_LEN..)
        .ok_or_else(|| malformed("the file is shorter than its header"))?;

    if blake3::hash(body).as_bytes() != &header[DIGEST_AT..HEADER_LEN] {
        return Err(malformed("the body does not hash to the digest it carries"));
    }

    let mut reader = Reader { bytes: body, at: 0 };
    let article_count = usize::try_from(reader.u32()?)
        .map_err(|_| malformed("the article count does not fit this machine"))?;

    if article_count == 0 {
        return Err(malformed("the book has no articles"));
    }

    if article_count > reader.remaining() {
        return Err(malformed("the article count is larger than the file"));
    }

    let mut articles = Vec::with_capacity(article_count);
    for _ in 0..article_count {
        articles.push(read_article(&mut reader)?);
    }

    let term_count = usize::try_from(reader.u32()?)
        .map_err(|_| malformed("the term count does not fit this machine"))?;

    if term_count > reader.remaining() {
        return Err(malformed("the term count is larger than the file"));
    }

    let mut terms: BTreeMap<String, TermFields> = BTreeMap::new();
    for _ in 0..term_count {
        let term = reader.string()?;
        if term.is_empty() {
            return Err(malformed("a term is empty"));
        }

        let mut term_fields = fields();
        for field in Field::ALL {
            let postings = &mut term_fields[field.index()];
            let count = usize::try_from(reader.u32()?)
                .map_err(|_| malformed("a posting count does not fit this machine"))?;

            if count > reader.remaining() {
                return Err(malformed("a posting count is larger than the file"));
            }

            postings.reserve(count);
            let mut previous = None;
            for _ in 0..count {
                let article = usize::try_from(reader.u32()?)
                    .map_err(|_| malformed("an article index does not fit this machine"))?;

                if article >= article_count {
                    return Err(malformed(
                        "a posting names an article the file does not hold",
                    ));
                }

                if previous.is_some_and(|previous| article <= previous) {
                    return Err(malformed("a term's postings are not in article order"));
                }
                previous = Some(article);

                let tf = reader.u32()?;
                if tf == 0 {
                    return Err(malformed("a posting counts the term zero times"));
                }

                postings.push(Posting { article, tf });
            }
        }

        if terms.insert(term, term_fields).is_some() {
            return Err(malformed("a term appears twice in the index"));
        }
    }

    if reader.remaining() != 0 {
        return Err(malformed("the file holds bytes after its index"));
    }

    Ok(Decoded { articles, terms })
}

/// Reads one article, in the order the format writes one.
fn read_article(reader: &mut Reader<'_>) -> Result<crate::Article, BookError> {
    let name = reader.string()?;
    let title = reader.string()?;
    let description = reader.string()?;
    let keyword_count = usize::try_from(reader.u32()?)
        .map_err(|_| malformed("a keyword count does not fit this machine"))?;

    if keyword_count > reader.remaining() {
        return Err(malformed("a keyword count is larger than the file"));
    }

    let mut keywords = Vec::with_capacity(keyword_count);
    for _ in 0..keyword_count {
        keywords.push(reader.string()?);
    }

    let markdown = reader.string()?;
    let plain = reader.string()?;

    Ok(crate::Article::new(
        name,
        title,
        description,
        keywords,
        markdown,
        plain,
    ))
}

/// Every term of every field of every article, with the postings it appears in.
fn collect_terms(ordered: &[&SourceArticle]) -> BTreeMap<String, TermFields> {
    let mut terms: BTreeMap<String, TermFields> = BTreeMap::new();

    for (index, article) in ordered.iter().enumerate() {
        for field in Field::ALL {
            let text = field.text(
                &article.name,
                &article.title,
                &article.description,
                &article.keywords,
                &article.plain,
            );

            let mut counts: BTreeMap<String, u32> = BTreeMap::new();
            for term in token::terms(&text) {
                let count = counts.entry(term.text).or_default();
                *count = count.saturating_add(1);
            }

            for (term, tf) in counts {
                let postings = terms.entry(term).or_insert_with(fields);
                postings[field.index()].push(Posting { article: index, tf });
            }
        }
    }

    terms
}

/// Narrows a count to the format's own number type, naming what could not be named.
fn narrow(value: usize, what: &str) -> Result<u32, BookError> {
    u32::try_from(value).map_err(|_| BookError::Oversized {
        detail: format!("{what} does not fit in four bytes"),
    })
}

/// What a book that cannot be decoded says about itself.
fn malformed(detail: &str) -> BookError {
    BookError::Malformed {
        detail: detail.to_owned(),
    }
}

/// The growing buffer a book is written into.
#[derive(Default)]
struct Encoder {
    /// The bytes written so far.
    bytes: Vec<u8>,
}

impl Encoder {
    /// Writes a `u32` in the format's own order.
    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    /// Writes a length-prefixed string.
    fn string(&mut self, text: &str) -> Result<(), BookError> {
        self.u32(narrow(text.len(), "a piece of text")?);
        self.bytes.extend_from_slice(text.as_bytes());

        Ok(())
    }

    /// Gives back what was written.
    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

/// A bounds-checked walk over a book's body.
struct Reader<'a> {
    /// The body being read.
    bytes: &'a [u8],
    /// How far the walk has got.
    at: usize,
}

impl<'a> Reader<'a> {
    /// How many bytes are left.
    const fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.at)
    }

    /// Takes the next `length` bytes, or refuses when the file ends first.
    fn take(&mut self, length: usize) -> Result<&'a [u8], BookError> {
        let end = self
            .at
            .checked_add(length)
            .ok_or_else(|| malformed("a length runs past the end of the file"))?;

        let taken = self
            .bytes
            .get(self.at..end)
            .ok_or_else(|| malformed("the file ends inside a length"))?;

        self.at = end;

        Ok(taken)
    }

    /// Reads a `u32`.
    fn u32(&mut self) -> Result<u32, BookError> {
        let bytes = self.take(4)?;
        let array: [u8; 4] = bytes
            .try_into()
            .map_err(|_| malformed("a number is not four bytes"))?;

        Ok(u32::from_be_bytes(array))
    }

    /// Reads a length-prefixed string.
    fn string(&mut self) -> Result<String, BookError> {
        let length = usize::try_from(self.u32()?)
            .map_err(|_| malformed("a string's length does not fit this machine"))?;
        let bytes = self.take(length)?;

        String::from_utf8(bytes.to_vec()).map_err(|_| malformed("a piece of text is not UTF-8"))
    }
}

#[cfg(test)]
mod tests {
    use super::{FIELD_COUNT, MAGIC, decode, encode};
    use crate::{Field, SourceArticle};

    fn article(name: &str) -> SourceArticle {
        SourceArticle {
            name: name.to_owned(),
            title: format!("The {name} article"),
            description: format!("about {name}"),
            keywords: vec![name.to_owned()],
            markdown: format!("# The {name} article\n\nabout {name}"),
            plain: format!("The {name} article about {name}"),
        }
    }

    #[test]
    fn a_book_is_written_and_read_back_as_it_was() {
        let articles = vec![article("beta"), article("alpha")];
        let bytes = encode(&articles).expect("the book encodes");
        let decoded = decode(&bytes).expect("the book decodes");

        assert_eq!(decoded.articles.len(), 2);
        // The file is written in name order, whatever order the articles arrive in.
        assert_eq!(decoded.articles[0].name(), "alpha");
        assert_eq!(decoded.articles[1].name(), "beta");
        assert_eq!(decoded.articles[1].title(), "The beta article");
        assert_eq!(
            decoded.articles[1].markdown(),
            "# The beta article\n\nabout beta"
        );
    }

    #[test]
    fn the_same_articles_are_the_same_bytes() {
        let one = encode(&[article("alpha"), article("beta")]).expect("the book encodes");
        let two = encode(&[article("beta"), article("alpha")]).expect("the book encodes");

        assert_eq!(one, two);
        assert_eq!(&one[..MAGIC.len()], &MAGIC);
    }

    #[test]
    fn a_changed_article_changes_the_bytes() {
        let mut changed = article("alpha");
        changed.plain.push_str(" and something else");

        let one = encode(&[article("alpha")]).expect("the book encodes");
        let two = encode(&[changed]).expect("the book encodes");

        assert_ne!(one, two);
    }

    #[test]
    fn a_book_with_two_articles_of_one_name_is_refused() {
        let error = encode(&[article("alpha"), article("alpha")]).expect_err("the book is refused");

        assert!(matches!(
            error,
            crate::BookError::Duplicate { ref name } if name == "alpha"
        ));
    }

    #[test]
    fn a_book_with_nothing_in_it_is_refused() {
        assert!(matches!(
            encode(&[]).expect_err("the book is refused"),
            crate::BookError::Empty
        ));
    }

    #[test]
    fn a_file_cut_short_is_refused() {
        let bytes = encode(&[article("alpha")]).expect("the book encodes");

        for shorter in [0, 7, super::HEADER_LEN, bytes.len() - 1] {
            assert!(
                decode(&bytes[..shorter]).is_err(),
                "a book of {shorter} bytes should be refused"
            );
        }
    }

    #[test]
    fn a_body_that_does_not_hash_is_refused() {
        let mut bytes = encode(&[article("alpha")]).expect("the book encodes");
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;

        assert!(decode(&bytes).is_err());
    }

    #[test]
    fn a_format_version_this_build_does_not_write_is_refused() {
        let mut bytes = encode(&[article("alpha")]).expect("the book encodes");
        bytes[super::VERSION_AT..super::VERSION_AT + 4].copy_from_slice(&9u32.to_be_bytes());

        assert!(matches!(
            decode(&bytes),
            Err(crate::BookError::Unsupported { version: 9 })
        ));
    }

    #[test]
    fn every_field_keeps_its_own_postings() {
        let bytes = encode(&[article("alpha")]).expect("the book encodes");
        let decoded = decode(&bytes).expect("the book decodes");

        assert_eq!(Field::ALL.len(), FIELD_COUNT);
        for field in Field::ALL {
            assert!(
                decoded
                    .terms
                    .values()
                    .any(|fields| !fields[field.index()].is_empty()),
                "no term was indexed for {}",
                field.label()
            );
        }
    }
}
