# rorolala-dev-book-index

## Introduction

Reads the Markdown articles under `book/` and writes them, with the index over them, as one searchable file at `build/docs/builtin.book`, which is what the command line and the Desktop program open.

## Boundary

It owns reading the front matter, folding Markdown into the plain text the search reads, and keeping the raw Markdown beside it. The file format and the query belong to `modules/book-search`, which reads what this tool writes. Where the file is laid out in an export belongs to `rola-build`, which calls this crate for `cargo dev book` and for the copies an export carries; this tool names the two paths once so the two cannot disagree.
