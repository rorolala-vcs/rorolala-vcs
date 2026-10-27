# rola-define-format

The `rola.lock` format: the materials a project places, and the sources their content can be
fetched from.

A lock is what the git support reads to put a project's materials back. Each line names a path in
the project and the content that belongs there, and the lines at the top name the Vaults that
content can be fetched from. It is generated — `rola.toml` is the file a person writes — so its
grammar is deliberately small: no patterns, no conditions, no inheritance, no quoting.

```text
? FORMAT 1
? VAULT origin rola://127.0.0.1:7717/default
/Assets/hero.psd blake3:7c3a1e9b4f2d8a6c5e0b3f7d9a1c4e628b5f0d3a7c9e2b6f4a8d1c3e5b7f9a2d
```

Reading is lenient in one place and strict in the rest: a carriage return before the newline is
accepted, because a checkout on Windows may put one there, and everything the grammar does not name
is refused rather than guessed at. Writing has one spelling of every lock, so two locks holding the
same materials are the same bytes and a diff shows what changed rather than how a tool walked it.

This crate is the only place a lock is read or written. It holds no files, no network and no git:
the caller owns where the text came from and where it goes.
