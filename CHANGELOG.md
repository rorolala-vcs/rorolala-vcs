# CHANGELOG

This document records all Changelog entries for Rorolala VCS starting from 0.1.0, managed by the `cargo changelog` command built into this project.

# 0.1.3 (Unreleased)

## Bugfix

1. [`app`] [`rola`] [`pager`] Never page a run that is not talking to a terminal — [See details](dev/CHANGELOG-DETAILS.md#1-app-rola-pager-never-page-a-run-that-is-not-talking-to-a-terminal)
2. [`utils`] [`cli-theme`] [`rendering`] Draw a three-dash Markdown rule as a rule — [See details](dev/CHANGELOG-DETAILS.md#2-utils-cli-theme-rendering-draw-a-three-dash-markdown-rule-as-a-rule)

## Feature

1. [`app`] [`rola`] [`docs`] Read and search the built-in book from the command line — [See details](dev/CHANGELOG-DETAILS.md#1-app-rola-docs-read-and-search-the-built-in-book-from-the-command-line)
2. [`modules`] [`book-search`] Search the built-in book by word, with the matched fragments — [See details](dev/CHANGELOG-DETAILS.md#2-modules-book-search-search-the-built-in-book-by-word-with-the-matched-fragments)
3. [`utils`] [`cli-theme`] [`rendering`] Lay out a Markdown document for reading — [See details](dev/CHANGELOG-DETAILS.md#3-utils-cli-theme-rendering-lay-out-a-markdown-document-for-reading)
4. [`utils`] [`cli-theme`] [`table`] A full-width listing table for the command line — [See details](dev/CHANGELOG-DETAILS.md#4-utils-cli-theme-table-a-full-width-listing-table-for-the-command-line)
5. [`utils`] [`cli-theme`] [`theme`] Choose the Pretty theme where the terminal can be detected — [See details](dev/CHANGELOG-DETAILS.md#5-utils-cli-theme-theme-choose-the-pretty-theme-where-the-terminal-can-be-detected)

## Refactor

1. [`utils`] [`cli-theme`] [`style`] Keep the Pretty line prefixes the same as the Simple ones — [See details](dev/CHANGELOG-DETAILS.md#1-utils-cli-theme-style-keep-the-pretty-line-prefixes-the-same-as-the-simple-ones)

# 0.1.2 (2026-10-29)

## Optimized

1. **\[BREAKING!\]** [`modules`] Carry a sync's bytes as bytes all the way down, from the packet to the pack — [See details](dev/CHANGELOG-DETAILS.md#1-breaking-modules-carry-a-syncs-bytes-as-bytes-all-the-way-down-from-the-packet-to-the-pack)

## Feature

1. [`app`] [`rola`] Read, select and fetch the work by the managed ignore rules — [See details](dev/CHANGELOG-DETAILS.md#1-app-rola-read-select-and-fetch-the-work-by-the-managed-ignore-rules)
2. [`app`] [`rola`] Speak every action to a Vault behind SSH, one ssh per action — [See details](dev/CHANGELOG-DETAILS.md#2-app-rola-speak-every-action-to-a-vault-behind-ssh-one-ssh-per-action)
3. [`app`] [`ROLALauncher`] Read the scp spelling of a Vault address in the window that creates a Workspace — [See details](dev/CHANGELOG-DETAILS.md#3-app-rolalauncher-read-the-scp-spelling-of-a-vault-address-in-the-window-that-creates-a-workspace)
4. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Hide what a Workspace's ignore rules govern in the browser — [See details](dev/CHANGELOG-DETAILS.md#4-app-rorolaladesktop-plugins-rorolala-vcs-hide-what-a-workspaces-ignore-rules-govern-in-the-browser)
5. [`modules`] Manage ignore rules in one directory, and let a Vault hand rules down — [See details](dev/CHANGELOG-DETAILS.md#5-modules-manage-ignore-rules-in-one-directory-and-let-a-vault-hand-rules-down)
6. [`modules`] Reach a Vault over an SSH session, with no port bound at either end — [See details](dev/CHANGELOG-DETAILS.md#6-modules-reach-a-vault-over-an-ssh-session-with-no-port-bound-at-either-end)

# 0.1.1 (2026-10-28)

## Bugfix

1. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Keep a tree's scroll and chosen step when the files change under it — [See details](dev/CHANGELOG-DETAILS.md#1-app-rorolaladesktop-plugins-file-system-tree-keep-a-trees-scroll-and-chosen-step-when-the-files-change-under-it)
2. [`modules`] [`layout`] Read a shard's snapshot and its log under one lock, and fold a log that has outgrown them — [See details](dev/CHANGELOG-DETAILS.md#2-modules-layout-read-a-shards-snapshot-and-its-log-under-one-lock-and-fold-a-log-that-has-outgrown-them)
3. [`modules`] [`tree-analyze`] [`cache`] Keep a shared analysis cache whole while another reading writes it — [See details](dev/CHANGELOG-DETAILS.md#3-modules-tree-analyze-cache-keep-a-shared-analysis-cache-whole-while-another-reading-writes-it)

## Optimized

1. **\[BREAKING!\]** [`modules`] [`daemon`] Bring the chain under a version from a Vault in one exchange — [See details](dev/CHANGELOG-DETAILS.md#1-breaking-modules-daemon-bring-the-chain-under-a-version-from-a-vault-in-one-exchange)
2. **\[BREAKING!\]** [`modules`] [`daemon`] Open the Vault's Layout once per action and write each run as one batch — [See details](dev/CHANGELOG-DETAILS.md#2-breaking-modules-daemon-open-the-vaults-layout-once-per-action-and-write-each-run-as-one-batch)
3. **\[BREAKING!\]** [`modules`] [`daemon`] [`fetch-layout`] Carry the Vault's Layout as the whole it holds, and pass over a fetch whose copy is already it — [See details](dev/CHANGELOG-DETAILS.md#3-breaking-modules-daemon-fetch-layout-carry-the-vaults-layout-as-the-whole-it-holds-and-pass-over-a-fetch-whose-copy-is-already-it)
4. [`app`] [`rola`] Write a run's ownership changes into the fetched copy once — [See details](dev/CHANGELOG-DETAILS.md#4-app-rola-write-a-runs-ownership-changes-into-the-fetched-copy-once)
5. [`app`] [`rola`] [`retrack`] Read a retrack's chain in one exchange rather than one per object — [See details](dev/CHANGELOG-DETAILS.md#5-app-rola-retrack-read-a-retracks-chain-in-one-exchange-rather-than-one-per-object)
6. [`app`] [`rola`] [`track`] Pack an index with the objects a recording made, and gather the packs when due — [See details](dev/CHANGELOG-DETAILS.md#6-app-rola-track-pack-an-index-with-the-objects-a-recording-made-and-gather-the-packs-when-due)
7. [`modules`] [`daemon`] Resolve a Vault once for as long as its files say the same thing — [See details](dev/CHANGELOG-DETAILS.md#7-modules-daemon-resolve-a-vault-once-for-as-long-as-its-files-say-the-same-thing)
8. [`modules`] [`daemon`] Turn off Nagle on both ends of a session's connections — [See details](dev/CHANGELOG-DETAILS.md#8-modules-daemon-turn-off-nagle-on-both-ends-of-a-sessions-connections)
9. [`modules`] [`daemon`] [`sync-all`] Stop reading the store twice on each side of a sync — [See details](dev/CHANGELOG-DETAILS.md#9-modules-daemon-sync-all-stop-reading-the-store-twice-on-each-side-of-a-sync)
10. [`modules`] [`daemon`] [`sync-index`] Carry an index sync as packets and write it as one pack — [See details](dev/CHANGELOG-DETAILS.md#10-modules-daemon-sync-index-carry-an-index-sync-as-packets-and-write-it-as-one-pack)
11. [`modules`] [`protocol`] Write a small value as one record on the session — [See details](dev/CHANGELOG-DETAILS.md#11-modules-protocol-write-a-small-value-as-one-record-on-the-session)
12. [`modules`] [`vcs`] [`index`] Keep a pack's index in memory, and answer presence for a batch from one reading — [See details](dev/CHANGELOG-DETAILS.md#12-modules-vcs-index-keep-a-packs-index-in-memory-and-answer-presence-for-a-batch-from-one-reading)
13. [`modules`] [`vcs`] [`index`] Write a run of index objects as one pack — [See details](dev/CHANGELOG-DETAILS.md#13-modules-vcs-index-write-a-run-of-index-objects-as-one-pack)

## Feature

1. **\[BREAKING!\]** [`app`] [`RorolalaDesktop`] [`docking`] Replace the dock area's five fixed regions with a recursive layout tree — [See details](dev/CHANGELOG-DETAILS.md#1-breaking-app-rorolaladesktop-docking-replace-the-dock-areas-five-fixed-regions-with-a-recursive-layout-tree)
2. [`app`] [`git-rola`] Add git rola, the git support program — [See details](dev/CHANGELOG-DETAILS.md#2-app-git-rola-add-git-rola-the-git-support-program)
3. [`app`] [`rola-daemon-gui`] Add the GTK 4 window over the daemon on Linux — [See details](dev/CHANGELOG-DETAILS.md#3-app-rola-daemon-gui-add-the-gtk-4-window-over-the-daemon-on-linux)
4. [`dev`] [`rola-build`] [`publish`] Package the Linux hand-off as deb, rpm, Arch and AppImage — [See details](dev/CHANGELOG-DETAILS.md#4-dev-rola-build-publish-package-the-linux-hand-off-as-deb-rpm-arch-and-appimage)
5. [`modules`] [`define-format`] Add rola-define-format, the rola.lock format — [See details](dev/CHANGELOG-DETAILS.md#5-modules-define-format-add-rola-define-format-the-rolalock-format)
6. [`modules`] [`git-support`] Add rola-git-support for project files and the work tree — [See details](dev/CHANGELOG-DETAILS.md#6-modules-git-support-add-rola-git-support-for-project-files-and-the-work-tree)

# 0.1.0 (2026-10-26)

## Feature

1. [`new`] Add the first usable release of the Rorolala VCS suite — [See details](dev/CHANGELOG-DETAILS.md#1-new-add-the-first-usable-release-of-the-rorolala-vcs-suite)
