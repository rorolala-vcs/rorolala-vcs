# rorolala-book-search

The built-in book, searchable: one file holds the articles and the index over them, and this crate
is what opens it and answers a query.

The file is not written here. `cargo dev book` (`dev/book-index`) reads `book/**/*.md`, and writes
`build/docs/builtin.book`; what this crate carries is the format those two agree on, the tokenizer
both sides cut text with, and the search a program asks. The reading and writing halves are one
crate so that the file a build produces and the file a program opens cannot drift apart.

## What a query means

A query is cut into the same terms the book was indexed by:

- text separated by spaces is cut into words, lowercased and compatibility-normalized, so `Rola`
  and `ＲＯＬＡ` are the same term;
- Chinese-style text — Han, kana, hangul — is cut into each character and each adjacent pair,
  because it is written without spaces. The characters let a one-character query find its line;
  the pairs let a longer query find its phrase.

An article is a hit only when **every** term of the query is in it, in any of its fields: name,
title, description, keywords or content. A query whose terms are not all there finds nothing,
which is what makes `版本控制` ask for the phrase rather than for its four characters in four
places. A query that is all punctuation has no terms and finds nothing.

The hits are ranked by BM25 over the fields, with a hit in the title worth more than one in the
body (see `Field::weight`). The name breaks a tie, so the same query over the same book answers in
the same order every time.

## What a hit carries

Each hit is the article's name, title and description, a score, and at most
[`MAX_SNIPPETS_PER_HIT`] fragments. A fragment is cut from the first field that matched — the
content if the word is in the body, and the description or title when the word is only there — and
ranges say which characters of the fragment matched:

```rust,no_run
# use rorolala_book_search::BookSearch;
# use std::path::Path;
# let path = Path::new("builtin.book");
let book = BookSearch::open(path).expect("the book opens");
let hits = book.search("版本控制", 10);

for hit in &hits {
    for snippet in &hit.snippets {
        // Style `snippet.text` at `snippet.ranges`, which count characters rather than bytes.
        let _ = (&snippet.field, &snippet.text, &snippet.ranges);
    }
}
```

Ranges are counted in characters and include the start but not the end, so a caller can split the
text into styled and unstyled runs whatever encoding it holds the text in. A fragment cut shorter
than its field carries `…` at the ends it was cut from, and the ranges count from the marked text.

## Opening a book

`BookSearch::open` reads and checks the file; `BookSearch::locate` walks up from a program's own
directory to the root that holds both `bin/` and `docs/`, and opens `docs/builtin.book` there. Both
the command line program (`bin/rola`) and the Desktop program (`bin/desktop/…`) are found by the
same walk, which is why the file is laid beside `bin/` rather than inside it.

The file is signed: its header carries a BLAKE3 digest of its body, and nothing past the header is
read until the body hashes to what the header says. A file cut short, written over, or written by a
format version this build does not know is refused rather than searched.

## The file

```text
magic        b"ROLABOOK"             8
version      u32, big endian         4
digest       BLAKE3 of the body      32
body:
  articles   u32 count, then per article:
               name, title, description   length-prefixed UTF-8
               keywords                   u32 count, then length-prefixed UTF-8
               markdown, plain            length-prefixed UTF-8
  terms      u32 count, then per term, sorted:
               term                       length-prefixed UTF-8
               per field in `Field::ALL`  u32 count, then postings:
                                            article index u32, term count u32
```

Both article text streams are kept: `markdown` is the article as it was written, for a reader that
draws it as Markdown, and `plain` is the same article with the syntax taken away, which is what the
index is built from and what fragments are cut out of. Counts and offsets are big endian, and the
terms are written in sorted order, so the same articles produce the same bytes on every machine.

## The C ABI

`open_rola_book_search` and `locate_rola_book_search` hand out a `RolaBookSearch`;
`search_rola_book` answers with a `RolaBookResults`, and each `rola_book_results_hit` hands out a
`RolaBookHit` with the fragments under `rola_book_hit_snippet`. Every handle is released with its
own `free_*`, and every string with `free_string`. The hand-written C# wrapper in
`modules/rola-sharp` is the shape a Desktop caller is meant to use; the exports are what a C
program meets.
