# rola-changelog

The editor behind `cargo changelog`.

This is a development tool: it records what changed in `CHANGELOG.md` between releases, and it sets
the workspace version the changelog is written under. It is reached through the alias in
`.cargo/config.toml`, which names this manifest because the crate is a workspace of its own — the
reason `dev/build` gives, with the picker's `arg!` being the feature that must not leak into the
programs.

## Commands

- `cargo changelog set-version <VERSION>` — replace `[workspace.package].version` in the root
  `Cargo.toml`, then run `cargo check --workspace` so the member versions in `Cargo.lock` move with
  it. Both files are restored if that check fails.
- `cargo changelog get-version` — print the workspace version on stdout. When there is none,
  nothing is printed and the exit status is `EC_NO_VERSION` (10).
- `cargo changelog write-reset` — reset `__CHANGELOG_DRAFT.md` to its template.
- `cargo changelog write` — merge the draft into the current version's `(Unreleased)` section. A
  missing draft is a success that does nothing. The original is copied to
  `__CHANGELOG_BACKUP.md`, the merge is written to `__CHANGELOG_REWRITE.md`, and that file is
  renamed over `CHANGELOG.md` so a reader never sees half a rewrite.
- `cargo changelog archive` — replace the newest `(Unreleased)` marker with today's UTC date. A
  sealed version is never written to again; a new one starts after `set-version`.

Every command that writes the tree refuses a dirty one unless `--allow-dirty` is given. The draft,
backup and rewrite files are named `__…` and are therefore ignored by the repository.

## The draft

`__CHANGELOG_DRAFT.md` is front matter and a body, like a skill:

```markdown
---
summary: Fix the blocking read in checkin
category: bugfix
scope: ["app", "rola", "checkin"]
breaking: true
---

The body of the entry.
```

`category` is one of `bugfix`, `optimized`, `feature`, `refactor`. `breaking: true` marks the
change as incompatible. `scope` is the hierarchy the change touches, primary first; it may be a
list or a single string. The scope follows the directory tree; the changelog-entry skill
(`.agents/skills/changelog-entry/SKILL.md`) is the project's guide to choosing it. The body may be
empty.

## The changelog

```markdown
# 0.1.0 (Unreleased)

## Bugfix

### 1. **\[BREAKING!\]** [`cli`] Summary

Content.
```

A version emits only the categories that have entries; a block with nothing in it is not written.
Within a category and version, entries are rewritten in this order on every `write` and `archive`:

1. breaking entries first;
2. then by the scope hierarchy, primary scope, secondary scope, tertiary scope…, each compared
   case-insensitively;
3. then by summary.

Numbering is reassigned from 1 after that order. Archived versions keep their content; only the
version being written and the one being archived are parsed and rewritten.

The tool does not maintain a table of contents or translate its output: it is a development tool,
not one of the programs.
