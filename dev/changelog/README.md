# rola-changelog

The editor behind `cargo changelog`.

This is a development tool: it records what changed between releases in
`dev/CHANGELOG-DETAILS.md`, generates the summary `CHANGELOG.md` from it, and sets the workspace
version the details are written under. It is reached through the alias in `.cargo/config.toml`,
which names this manifest because the crate is a workspace of its own — the reason `dev/build`
gives, with the picker's `arg!` being the feature that must not leak into the programs.

## Files

`dev/CHANGELOG-DETAILS.md` is the changelog a person writes: every version, its categories, and the
entries under them with their bodies. `CHANGELOG.md` at the repository root is generated from it:
the title, versions and categories are the details', and each entry is a numbered list item whose
title links to the paragraph it holds in the details.

Only the details are edited by hand. `write` and `archive` rewrite the details and generate the
summary in the same run, and `render-summary` regenerates it alone after a hand edit; the summary is
committed with them and never edited.

## Commands

- `cargo changelog set-version <VERSION>` — replace `[workspace.package].version` in the root
  `Cargo.toml`, then run `cargo check --workspace` so the member versions in `Cargo.lock` move with
  it. Both files are restored if that check fails.
- `cargo changelog get-version` — print the workspace version on stdout. When there is none,
  nothing is printed and the exit status is `EC_NO_VERSION` (10).
- `cargo changelog write-reset` — reset `__CHANGELOG_DRAFT.md` to its template.
- `cargo changelog write` — merge the draft into the current version's `(Unreleased)` section in
  `dev/CHANGELOG-DETAILS.md`, then generate `CHANGELOG.md` from it. A missing draft is a success
  that does nothing. The details are copied to `__CHANGELOG_DETAILS_BACKUP.md` and merged in
  `__CHANGELOG_DETAILS_REWRITE.md` before the rename; the summary is copied to
  `__CHANGELOG_BACKUP.md` and assembled in `__CHANGELOG_REWRITE.md` the same way. A file whose
  content would not change is left alone.
- `cargo changelog render-summary` — regenerate `CHANGELOG.md` from the details, writing it only
  when its bytes would change. It does not refuse a dirty tree: a hand edit of the details is the
  input it renders.
- `cargo changelog archive` — replace the newest `(Unreleased)` marker in the details with today's
  UTC date, then generate `CHANGELOG.md` from it. A sealed version is never written to again; a new
  one starts after `set-version`.

`write`, `archive` and `set-version` refuse a dirty tree unless `--allow-dirty` is given.
`render-summary` does not: it writes only the generated summary, and a hand edit of the details is
what it is rendering. The draft, backup and rewrite files are named `__…` and are therefore ignored
by the repository.

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
(`.agents/skills/changelog-entry/SKILL.md`) is the project's guide to whether a change needs an
entry at all and to choosing it. The body may be empty.

## The details

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

## The summary

```markdown
# 0.1.0 (Unreleased)

## Bugfix

1. **\[BREAKING!\]** [`cli`] Summary — [See details](dev/CHANGELOG-DETAILS.md#1-breaking-cli-summary)
```

The summary is a function of the details: the same title, versions and categories, with each
entry's body left out and its title linked to the entry's heading. The anchor is the one GitHub
gives that heading — lowercased, punctuation dropped, spaces turned into hyphens — so the link
lands on the paragraph in a rendered view of the details. Two headings that would share an anchor
get `-1`, `-2`, … on the later ones, as GitHub does. Generating the same details twice writes the
same summary.

The tool does not maintain a table of contents or translate its output: it is a development tool,
not one of the programs.

## The release

A tag is published by `.github/workflows/ci.yml`. The release body is the tagged version's own
section of `CHANGELOG.md` — its version heading, categories and the entries as the summary lists
them — followed by the people whose commits made up the range since the previous tag. Run
`cargo changelog render-summary` before tagging if the details were edited by hand, so the summary
the tag reads is current. The summary's links are relative to the repository root, which a release
body cannot resolve, so the release step rewrites each `dev/CHANGELOG-DETAILS.md#anchor` target to
`https://github.com/rorolala-vcs/rorolala-vcs/blob/<commit>/dev/CHANGELOG-DETAILS.md#anchor` at the
tag's commit. A tag whose section is missing or still marked `(Unreleased)` is refused rather than
released with an empty or unfinished note.
