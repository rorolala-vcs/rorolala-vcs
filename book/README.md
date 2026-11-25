---
title: "Contributing a Book"
description: "How to contribute an article to the built-in book, and what an article is made of."
keywords: contributing, book, article, docs
---

# Contributing

`book/` is the built-in book that `rola docs` reads. Every Markdown file under this directory is an
article, including this one, which is the book's own page about contributing to it.

## What can be contributed

- An article. It begins with a YAML front matter block holding `title`, `description` and
  `keywords`, and its body is plain text, because images and HTML are not recognised.
- A correction to an article under `book/`. `book/app/` describes what a program offers,
  `book/concepts/` the places and records it works with, `book/reference/` the words this book
  uses, and `book/vcs-knowledge/` the version control objects and the cuts a Store keeps them in.

## What it needs

Nothing beyond the Rust toolchain. The search file is built from the Markdown by `dev/book-index`.

## Checks

`cargo dev export-book` writes the search file to `build/docs/builtin.book`, and a build of the
workspace writes it with everything else. `cargo ci test` covers the indexer.
