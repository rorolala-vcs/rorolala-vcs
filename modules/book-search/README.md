# rorolala-book-search

## Introduction

The built-in book: the file format at `docs/builtin.book`, the tokenizer both sides cut text with, and the search that answers a query from it.

## Boundary

It owns opening, verifying and searching a book, the ranking of hits, the snippets cut from a matched field, and the C ABI a Desktop caller reaches them through. It does not write the file: `cargo dev book` (`dev/book-index`) builds it from `book/**/*.md`, and that build is not this crate. The C# wrapper is `modules/rola-sharp`, and drawing a hit is the caller's.
