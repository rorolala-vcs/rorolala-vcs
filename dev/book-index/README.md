# rorolala-dev-book-index

The built-in book, read from `book/` and written as one searchable file.

The articles are Markdown files with an optional YAML front matter block:

```markdown
---
title: "Installing Rorolala"
description: "Put the program on a machine and bind a workspace"
keywords: install, setup
---

# Installing Rorolala

The body, in Markdown.
```

`title`, `description` and `keywords` are what the search shows and the front matter names them;
`keywords` is one comma-separated line or a YAML list. A file that leaves the block out is still an
article: its title is its first heading, or its file name when it has none.

The article's name is its path under `book/`, `/`-separated, without the extension:
`getting-started/install.md` is the article `getting-started/install`. Names are what a hit is
opened by, so two files cannot make one.

## Building it

```sh
cargo dev book
```

That reads `book/` and writes `build/docs/builtin.book`. The file is laid beside `bin/` rather than
inside it, so both the command line program (`bin/rola`) and the Desktop program
(`bin/desktop/…`) find it by walking up to the root that holds the two directories.

`cargo dev export` and `cargo dev desktop` build it themselves, so a layout a person runs has the
file in it; `cargo dev publish` packages what the export laid down, and every package carries it.

The program takes the two paths when a caller is laying a build out somewhere else:

```sh
cargo run -p rorolala-dev-book-index -- <book-dir> <output-file>
```

## What it writes

One file, holding the articles and the index over them, in the format
[`modules/book-search`](../../modules/book-search) reads. What is indexed is the plain text of the
article — Markdown syntax taken away and whitespace collapsed — and the raw Markdown is kept beside
it so a reader can draw the article as it was written.

A file or directory whose name begins with `__` is left out: that is where this tree keeps a draft.
