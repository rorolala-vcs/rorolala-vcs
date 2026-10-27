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

### 2. [`modules`] [`layout`] Read a shard's snapshot and its log under one lock, and fold a log that has outgrown them

Opening a layout read a shard's snapshot and then its log as two reads, and a compaction between the two
was a change lost: the snapshot read before a compaction and the log read after it are a shard with no
entries, since the compaction had already folded what the log held into a snapshot the open did not see.
A shard is now read whole — its snapshot and the log after it under the log's shared lock, which is the
lock a compaction holds exclusively for the whole of what it does. The open also builds what it answers
from the shards it just read rather than refreshing a second time, which is one read fewer of every
shard.

A compaction passes over a shard whose log holds no bytes: there is nothing to fold, and what it would
spend is a flush of a snapshot that is what is already there.

A Vault also folds its Layout's log into the snapshots when the log has outgrown them — past a
mebibyte, or more than what the snapshots hold — which is what keeps the reading an open does from
growing with the history a Vault has served. A compaction that fails is left to the next action rather
than failing the one that was asked for.

### 3. [`modules`] [`tree-analyze`] [`cache`] Keep a shared analysis cache whole while another reading writes it

The cache a reading of the tree writes down is now written beside itself and moved over the file that is
there, rather than written over it. One work is read by several readers at once — the desktop's Overview and
each listing of it read it on threads of their own — and a reader opens either the cache that was there or
the one being written, and never a file that is part of each.

A reader that caught the old file between the truncation and the new bytes took it for a cache written by
another build and started from no agreements at all. It then wrote the tree as it stands as what the Layout
agrees with, and the changes it was about to report — a file edited without holding it, say — were gone from
every reading after that until the files changed again.

## Optimized

### 1. **\[BREAKING!\]** [`modules`] [`daemon`] Bring the chain under a version from a Vault in one exchange

A retrack may move the pointer to a version the Vault holds and this side never fetched. What it needs for
such a version to be readable here is the chain under it - the version, the variant that produced it, the
words they are named by, and so on down to a version this index already holds - and reading it one object
per exchange was a connection and a handshake per object.

It is one exchange now: the Workspace says the hash it wants the chain of and what it already holds, the
Vault walks down until it meets something the Workspace has, and what it found crosses back together, to be
written here as one pack. The walk stops at the root version rather than failing on the variant a root
version names and does not have.

The action is `bring-remote-index`, id 17. **Breaking:** a daemon and a program have to be built together: a
program that asks for the action against a daemon which does not serve it is refused.

### 2. **\[BREAKING!\]** [`modules`] [`daemon`] Open the Vault's Layout once per action and write each run as one batch

The actions that change the Vault's Layout no longer open it once per entry, and no longer write one
change at a time. An ownership run — `hold`, `giveup`, and the two `rola layout` doors — opens the
Layout once, decides every `Uuid` against one batch, and commits it: one reading of the shards and one
flush per shard, where it used to be sixteen shard reads and a flush for every entry. The `Uuid`s cross
as one value and the outcomes come back as one, so a run of a thousand entries is two values instead of
a thousand round trips.

A write that is made of many entries — `write-remote-entries`, which a sync's push uses — is one batch
as well, and a create is one record rather than a path and an entry that a crash could part.

The batch is committed before the outcomes are answered, and the side that asked is told whether it was
written before it is told what became of each entry, so an outcome is never one the Vault did not keep.
A commit that fails is reported with what it failed with.

**Breaking:** what an ownership run and a many-entry write exchange has changed shape. Both ends run the
same action and have to be built together: a Vault served by an older daemon, or a program built before
this, does not understand the exchange.

### 3. **\[BREAKING!\]** [`modules`] [`daemon`] [`fetch-layout`] Carry the Vault's Layout as the whole it holds, and pass over a fetch whose copy is already it

A fetch no longer carries the Vault's Layout directory as it stands. It carries the layout written out
whole — every shard a snapshot of everything it holds, with an empty log after it — because a directory
read while another action writes or compacts is the two halves of two states, and what a copy is for is
one whole.

A fetch that asks with the state its copy already holds is answered "unchanged" and carries nothing, so
a run whose layout has not changed since the copy was taken pays a round trip rather than a directory.
What a layout is written out as depends on what it holds and not on how it is kept, so a compaction does
not make a copy stale; and because the state is read back from the copy, a copy that was written to
after it was fetched — an ownership change followed into it — answers for what it now holds rather than
for the files it arrived in.

The files of a generation are written without a flush each, and one flush of the generation as a whole
is asked for where the system has one. A machine cut off before the pointer moves loses this fetch, not
the copy that was there.

**Breaking:** what crosses for a fetch has changed shape, so a daemon and a program have to be built
together.

### 4. [`app`] [`rola`] Write a run's ownership changes into the fetched copy once

A run that moved ownership opened the fetched copy again for every entry and wrote one change for each:
opening a copy reads sixteen shards, and writing one change is a read and a flush, so a thousand files
was a thousand of each. The copy is opened once and written once, which is one batch for the whole run.
A copy that cannot be written is still reported, and the change the Vault made is already made. The two
`rola layout` doors that ask for ownership of the entries they name write the copy the same way.

### 5. [`app`] [`rola`] [`retrack`] Read a retrack's chain in one exchange rather than one per object

A retrack that goes to a version the Vault holds and this side never fetched read the chain one object at a
time, each its own connection and handshake. It asks for the whole chain in one exchange now, and what the
Vault brings is written as one pack here - one file and one index for a chain of any length.

What a run is told when the chain cannot be walked is unchanged: an object under the hash that is not a
version is refused as a level that is not a version, and a hash the chain could not go past is named.

### 6. [`app`] [`rola`] [`track`] Pack an index with the objects a recording made, and gather the packs when due

Every recording packed the index, and packing rewrites every object the index has ever held: the cost of
a run grew with the index for a saving that did not. A recording now writes the objects it makes as one
pack — one file and one index for the whole run rather than a file per object — and gathers what has
piled up only when the loose objects it leaves number a few hundred, or when it has left more than a
handful of packs behind. Until then what it wrote is valid where it is, and a later run or `rola pack`
settles it.

### 7. [`modules`] [`daemon`] Resolve a Vault once for as long as its files say the same thing

Every connection read the namespace table, the Vault's configuration and its root's, and walked to the root
Vault again. What a request's namespace resolves to is now kept beside the length and modification time of
the files it was read from, and made again only when one of them has changed: a Vault registered or
reconfigured while the daemon runs is still served without a restart, and a connection that names a Vault
nothing has touched pays a few stats instead of a table and two configurations.

What is kept is bounded, so a peer that names a different Vault every time cannot grow the daemon's memory
with it.

### 8. [`modules`] [`daemon`] Turn off Nagle on both ends of a session's connections

A value crosses as a length record and a payload record, and a request and its answer alternate on one
connection. Left to Nagle, the second record can wait for the first to be acknowledged, which is a round
trip a small action spends on nothing, so both ends now ask for `TCP_NODELAY` on the session's
connections.

### 9. [`modules`] [`daemon`] [`sync-all`] Stop reading the store twice on each side of a sync

A full sync asked each side what it could produce before it moved anything, and then asked the side that
took what arrived whether it could read it back: a reading of every content before any byte moved, and
another reading of everything that arrived before the sync could return.

What a side now lists is every key its store holds, read from the loose directory and the pack indexes,
and the presence the two answers feed is what decides each key. What arrives is checked against the key
it came under as it is taken, and the side that took it confirms what it holds by lookup rather than by
putting every content back together and hashing it — one reading of the store where there were two.

The one thing this makes stricter: a key a store holds but cannot produce — a manifest whose chunks are
gone — is now one both sides are asked for, and neither holds, so the run says so instead of leaving it
out of the sync in silence.

### 10. [`modules`] [`daemon`] [`sync-index`] Carry an index sync as packets and write it as one pack

An index sync carried one object per exchange, and the side taking it wrote one file per object. It now
carries a packet of objects per value, as a store sync does, and the side taking a direction writes the
whole of it as one pack: what was a file created, renamed and indexed for every object is one file and one
index for the run.

What the side taking a direction came to is said back as a status, and the objects it cannot produce are
named, so a direction that failed is not one either side calls done.

### 11. [`modules`] [`protocol`] Write a small value as one record on the session

A value is a length and then its payload, and each was a write of its own. The encrypted channel seals what
a write hands it, so every small value crossed as two records. A value whose length and bytes fit one
record is now written as one buffer, which is one record, one sealing and one write for every outcome, count
and packet a run exchanges.

A value too long for one record is left as the two writes it was: joining it would copy it for a boundary
the channel is going to make anyway.

### 12. [`modules`] [`vcs`] [`index`] Keep a pack's index in memory, and answer presence for a batch from one reading

A lookup that walked a pack read and decoded that pack's index every time it was asked, so asking
whether the index held a thousand keys decoded the same file a thousand times and paid more as the pack
grew. A pack's index is now kept with the length and modification time it was read with, and read again
only when the file it came from changed; the list of packs is kept the same way. Presence for a whole
batch is answered from one reading of the packs' indexes and one walk of the loose tree, so a batch of
fifty thousand keys is a decode and a walk rather than a hundred and fifty thousand looks at the
filesystem.

A listing that filters by kind no longer reads every object to find out what it is: the kind is the
first byte behind an object's frame, so a filtered listing reads that much of each object rather than the
object, its decode and its hash check. A listing that names no kind reads no object at all.

### 13. [`modules`] [`vcs`] [`index`] Write a run of index objects as one pack

A run of index objects - what a sync takes, what a recording makes - is now written through a batch: the
objects are collected and laid down as one pack with one index, rather than a file, a rename and a
directory entry for each of them. What a batch takes is what a transfer takes - an object is decoded and
held to the key it arrived under, and written as its canonical encoding.

A pack number is claimed by creating the pack's file, so two writers never take one number for two packs,
and a repacking leaves its consolidated pack under the number it claimed rather than moving it to zero:
moving one would be renaming a file another writer may be claiming.

## Feature

### 1. **\[BREAKING!\]** [`app`] [`RorolalaDesktop`] [`docking`] Replace the dock area's five fixed regions with a recursive layout tree

The main window's docks are no longer five fixed regions. Its arrangement is a tree: a leaf is a group
of tabs, any group may be split on any side, and a group made by splitting may itself be split, without
a depth limit. Every split has its own splitter and keeps the share of its first child as a ratio rather
than a width, so the arrangement follows the window. A split whose room cannot give both children their
least size has its splitter disabled instead of squeezing one out of reach, and the last dock leaving a
group takes the group with it, collapsing the split it was in.

A dragged dock is answered in two steps: carried into a group, the group is approached and five points
are drawn over it — four trapezoids along its edges, one for each way it may be split, and a centre
rectangle for joining its tabs — and the pointer is always on one of them. A drop on the centre joins
the group's tabs at the end of their order; a drop on an edge splits the group.

The group last clicked or focused is the active one, and a dock opened without a place of its own goes
there. The chosen tab of every group is remembered, and a dock moved into a window of its own returns to
the group it came from when that window goes.

The saved layout is version 2: the tree with a ratio for every split, the active group, and each group's
chosen tab. A version 1 file states placements and pixel widths, which cannot be turned into a tree
without inventing a window size; it is not converted — the starting arrangement is used, the log says
so, and the next save writes version 2. The starting arrangement is the old five-region one expressed as
a split of two groups, with the same tab orders and the same band.

**Breaking:** the plugin contract is 0.13.0. `DockPlacement` is now `DockSide`, the intent of a drop
rather than a region a dock sits in; `DockRegistration.DefaultPlacement` is `DefaultSide`; and `Create`
takes no side, because where a dock ends up is the layout tree's answer and is unknown when its view is
made. A plugin must be rebuilt against the new contract; one built against 0.12.0 is not loaded.

Resolves [#2](https://github.com/rorolala-vcs/rorolala-vcs/issues/2).

### 2. [`app`] [`git-rola`] Add git rola, the git support program

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

### 3. [`app`] [`rola-daemon-gui`] Add the GTK 4 window over the daemon on Linux

The daemon has a second window on Linux: `rola-daemon-gui`, built from C against GTK 4 and linked to
no Rust. It shows the same four pages as the Windows window — the daemon and its output, the namespace
registry, the user keys, and the Vaults' administrators — and every action behind them is still one
the command-line programs answer.

It reads the user's files where the Rust side does: `$ROLA_DATA_HOME` or `$XDG_DATA_HOME`, falling
back to `~/.local/share`, and then `rola/keys` and `rola/daemon/registry.toml`. `cargo dev export`
builds it on Linux into `build/bin/rola-daemon-gui`, after the programs it drives, so `rola-daemon`
is beside it. Building it needs GTK 4.10 or newer with its development files.

Stopping a daemon the window started asks it to terminate and falls back to killing it, because
`rola-daemon` has no stop command.

### 4. [`dev`] [`rola-build`] [`publish`] Package the Linux hand-off as deb, rpm, Arch and AppImage

`cargo dev publish` on Linux now lays the export out as the packages a distribution installs —
`rorolala-vcs`, `rorolala-desktop` and `rorolala-dev` — a `rorolala-full` meta package that names
all three, an all-in-one `rorolala` package that holds them, and a launcher-only AppImage. The
Desktop program and launcher are published self-contained, so the packages that hold them carry the
.NET runtime; the AppImage carries `rola`, `ROLALauncher` and `RororalaDesktop`, and no other
program.

The packages install the six programs, both desktop entries, the icons and completions, and a user
`rolad.service`. The unit is installed but never enabled or started by the package: whether the
daemon runs is its user's decision. Package metadata is read from the workspace's own `Cargo.toml`,
and nFPM and appimagetool are pinned beside the export.

### 5. [`modules`] [`define-format`] Add rola-define-format, the rola.lock format

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

### 6. [`modules`] [`git-support`] Add rola-git-support for project files and the work tree

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
