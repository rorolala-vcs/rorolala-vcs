# CHANGELOG

This document records all Changelog entries for Rorolala VCS starting from 0.1.0, managed by the `cargo changelog` command built into this project.

# 0.1.1 (Unreleased)

## Bugfix

### 1. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Keep a tree's scroll and chosen step when the files change under it

A tree told that the files changed is now read again in place instead of being built again. The rows are
read from disk, and the steps that were open and the step that was chosen are kept, so what a reader has
scrolled to stays where it was. A tree built again came back with its scroll at the top, and put the choice
back while its rows were still being made, which moved the scroll under the reader: in a tree longer than the
dock, the chosen step appeared to page up or down.

The choice is put back once the rows read again have been laid out, so a step the reader is already looking
at is not scrolled to as though it had just been chosen. A tree re-rooted, or read by other hide rules, is
still built again, since there the rows under the base are different rows.

### 2. [`modules`] [`tree-analyze`] [`cache`] Keep a shared analysis cache whole while another reading writes it

The cache a reading of the tree writes down is now written beside itself and moved over the file that is
there, rather than written over it. One work is read by several readers at once — the desktop's Overview and
each listing of it read it on threads of their own — and a reader opens either the cache that was there or
the one being written, and never a file that is part of each.

A reader that caught the old file between the truncation and the new bytes took it for a cache written by
another build and started from no agreements at all. It then wrote the tree as it stands as what the Layout
agrees with, and the changes it was about to report — a file edited without holding it, say — were gone from
every reading after that until the files changed again.

## Feature

### 1. [`app`] [`git-rola`] Add git rola, the git support program

`git-rola` is reached by Git as `git rola`, and is laid out beside the other programs so Git finds
it on the same path. It speaks git's manners rather than rola's: quiet on success, one
`git-rola: ...` line on stderr per failure, help on stdout, and `--` separating options from
paths.

- `init` writes `rola.toml` when the project has none, installs the `post-checkout`, `post-merge`
  and `post-rewrite` hooks (leaving a hook a person wrote alone), and puts an existing lock's
  materials in place.
- `check` compares every material with what `rola.lock` names and prints git's short status for
  the ones that differ (` M`) or are gone (` D`), exiting `EC_DIFFERENCES` when anything does.
- `restore` puts named materials back, reading each name as a pathspec; a name that selects
  nothing is refused before anything is written.
- `update` resolves each mapping's pin through the project's Vaults, brings the content into
  `.rola/storage/` and rewrites `rola.lock`; nothing is written unless every mapping resolves.
- `hook` is what the installed hooks run to keep the working tree in step with the lock.
- `checkout`, `switch`, `merge` and `pull` are wrapped: the materials are taken out, git runs, and
  they are put back.

Every command refuses a linked work tree, because a repository keeps one exclude file and reads it
in all of its work trees. Every word is translated from `i18n/` in the language `--lang` names or
the environment provides.

### 2. [`modules`] [`define-format`] Add rola-define-format, the rola.lock format

`rola.lock` names the materials a project places and the Vaults their content can be fetched
from. It is generated — `rola.toml` is the file a person writes — so the grammar is deliberately
small: a `? FORMAT 1` line, one `? VAULT <name> <url>` line per source, and one
`<path> blake3:<hash>` line per material, with no patterns, conditions, inheritance or quoting.

Reading accepts a carriage return before a newline, because a Windows checkout may put one there,
and refuses everything else the grammar does not name rather than guessing at it. Writing has one
spelling of every lock, so two locks holding the same materials are the same bytes. A lock naming
a format version this build does not know is refused whole.

Paths, Vault names and content hashes are validated types rather than strings, and the crate is
the only place a lock is read or written: it holds no files, no network and no git.

### 3. [`modules`] [`git-support`] Add rola-git-support for project files and the work tree

A project is a directory holding `rola.toml` — the Vaults, the features, and one `[[mapping]]` per
material — with the generated `rola.lock` beside it. This crate reads both, finds the project and
the git work tree it sits in, and answers what the programs ask of it.

It keeps the project's block of `.git/info/exclude` in step with the lock, so a placed material
stays invisible to git, and moves materials between the working tree and `.rola/git/staging/`,
recording what was last placed in `.rola/git/state.toml`. That area is covered by the same exclude
block and can be deleted: what is missing is put back from a source, and what is stale is corrected
on the next command.

Content is written through a `ContentSource` the caller supplies, so the crate holds no store, no
Vault and no network. A linked work tree is refused with `Error::LinkedWorkTree`, because git keeps
one exclude file for the whole repository and reads it in every work tree: one work tree's lock
must not be able to make another's materials visible to git.

# 0.1.0 (2026-10-26)

## Feature

### 1. [`new`] Add the first usable release of the Rorolala VCS suite

This change adds the Rorolala VCS 0.1.0 suite, as follows:

- Added the complete VCS subcommands to the `rola` CLI, including:
  - VCS subcommands track, retrack, checkin, fetch, sync, pack, status, entries, align, etc. (`app/cli/rola/src/`)
  - Commands related to `vcs-index`, `storage`, `layout`, and `ownership` (`app/cli/rola/src/vcs_index/`, `app/cli/rola/src/storage/`, `app/cli/rola/src/layout/`, `app/cli/rola/src/cmd_ownership.rs`)
- Added new Rust modules, including:
  - `modules/vcs` (versions, variants, creators, messages, index, and lookback) (`modules/vcs/`)
  - `modules/status` (status, version graph, merging, and ownership) (`modules/status/`)
  - `modules/tree-analyze` (scanning, fingerprints, tree diffs, and caching) (`modules/tree-analyze/`)
  - `modules/chunking` (chunking strategy registry, including the `blend-basic` and `psd-basic` corpora) (`modules/chunking/`)
- Added VCS-related remote actions to the daemon, including:
  - Creating/writing remote entries, fetching layouts, requesting/abandoning ownership, syncing storage and indexes, etc. (`modules/daemon/src/actions/`)
- Added the desktop-side `RorolalaVCSPlugin`, including:
  - Commands such as checkin/fetch/sync/track (`app/desktop/Plugins/RorolalaVCSPlugin/CheckinCommand.cs`, `Fetch.cs`, `Sync.cs`, `TrackCommand.cs`, etc.)
  - Account and ownership operations (`app/desktop/Plugins/RorolalaVCSPlugin/AccountActions.cs`, `OwnershipActions.cs`)
  - Version graph view, progress panel, and report view (`app/desktop/Plugins/RorolalaVCSPlugin/VersionGraphView.cs`, `ProgressPanel.cs`, `ReportView.cs`)
  - Sync watching and status badge (`app/desktop/Plugins/RorolalaVCSPlugin/SyncWatcher.cs`, `RorolalaStandingBadge.cs`)
- Added accompanying test projects and i18n resources, including:
  - Test cases for checkin/fetch/retrack/variants in the test projects `tests/vcs-index`, `tests/vcs-index-remote`, `tests/vcs-index-sync`, `tests/sync-chain`, and `tests/desktop-host` (`tests/`)
  - i18n resources `cmd_vcs_index*.yml` and `rorolala_vcs/strings.yml` (`app/cli/rola/i18n/`, `app/desktop/Plugins/RorolalaVCSPlugin/i18n/rorolala_vcs/strings.yml`)
