# CHANGELOG

This document records all Changelog entries for Rorolala VCS starting from 0.1.0, managed by the `cargo changelog` command built into this project.

# 0.3.0 (Unreleased)

## Bugfix

1. [`app`] [`cli`] [`tree`] [`storage`] Stage a tree's first content into packs. [See details](dev/CHANGELOG-DETAILS.md#1-app-cli-tree-storage-stage-a-trees-first-content-into-packs)
2. [`app`] [`modules`] [`workspace-analyze`] [`tree`] Find a tree nested inside another tree. [See details](dev/CHANGELOG-DETAILS.md#2-app-modules-workspace-analyze-tree-find-a-tree-nested-inside-another-tree)
3. [`app`] [`RorolalaDesktop`] [`cli`] [`tree`] [`storage`] Stage only the tree entries whose work changed. [See details](dev/CHANGELOG-DETAILS.md#3-app-rorolaladesktop-cli-tree-storage-stage-only-the-tree-entries-whose-work-changed)
4. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] Write a table's column widths after its layout pass. [See details](dev/CHANGELOG-DETAILS.md#4-app-rorolaladesktop-plugins-file-system-write-a-tables-column-widths-after-its-layout-pass)
5. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`icons`] Colour a tagged badge's ground from the theme's dark role. [See details](dev/CHANGELOG-DETAILS.md#5-app-rorolaladesktop-plugins-file-system-icons-colour-a-tagged-badges-ground-from-the-themes-dark-role)
6. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`icons`] Draw badge pictures at whole pixels and without a circular clip. [See details](dev/CHANGELOG-DETAILS.md#6-app-rorolaladesktop-plugins-file-system-icons-draw-badge-pictures-at-whole-pixels-and-without-a-circular-clip)
7. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`rorolala-vcs`] Explain every file badge on hover. [See details](dev/CHANGELOG-DETAILS.md#7-app-rorolaladesktop-plugins-file-system-rorolala-vcs-explain-every-file-badge-on-hover)
8. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`rorolala-vcs`] [`tree`] Report a tree's wrong change in its status and diff. [See details](dev/CHANGELOG-DETAILS.md#8-app-rorolaladesktop-plugins-file-system-rorolala-vcs-tree-report-a-trees-wrong-change-in-its-status-and-diff)
9. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Draw a chosen tree row once and centre its badges. [See details](dev/CHANGELOG-DETAILS.md#9-app-rorolaladesktop-plugins-file-system-tree-draw-a-chosen-tree-row-once-and-centre-its-badges)
10. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Keep a tree row's badges clear of its name. [See details](dev/CHANGELOG-DETAILS.md#10-app-rorolaladesktop-plugins-file-system-tree-keep-a-tree-rows-badges-clear-of-its-name)
11. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Keep a tree's colour stripe while the tree is resized. [See details](dev/CHANGELOG-DETAILS.md#11-app-rorolaladesktop-plugins-file-system-tree-keep-a-trees-colour-stripe-while-the-tree-is-resized)
12. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Keep a tree's guide layer out of the arrange pass while resizing. [See details](dev/CHANGELOG-DETAILS.md#12-app-rorolaladesktop-plugins-file-system-tree-keep-a-trees-guide-layer-out-of-the-arrange-pass-while-resizing)
13. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Keep the local pill on the draft while a file is edited. [See details](dev/CHANGELOG-DETAILS.md#13-app-rorolaladesktop-plugins-rorolala-vcs-keep-the-local-pill-on-the-draft-while-a-file-is-edited)
14. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Keep the message field's pointer and row steady. [See details](dev/CHANGELOG-DETAILS.md#14-app-rorolaladesktop-plugins-rorolala-vcs-keep-the-message-fields-pointer-and-row-steady)

## Optimized

1. [`modules`] [`chunking`] [`text`] Stop a text cut at the bound when a line runs past it. [See details](dev/CHANGELOG-DETAILS.md#1-modules-chunking-text-stop-a-text-cut-at-the-bound-when-a-line-runs-past-it)
2. [`modules`] [`inverse-index`] [`table`] Build the inverse index in the buffer it is written from. [See details](dev/CHANGELOG-DETAILS.md#2-modules-inverse-index-table-build-the-inverse-index-in-the-buffer-it-is-written-from)
3. [`modules`] [`protocol`] [`frame`] Read a carried frame into the buffer it is being put together in. [See details](dev/CHANGELOG-DETAILS.md#3-modules-protocol-frame-read-a-carried-frame-into-the-buffer-it-is-being-put-together-in)
4. [`modules`] [`storage`] Cut unclaimed binary content on its content, and write chunks in packs. [See details](dev/CHANGELOG-DETAILS.md#4-modules-storage-cut-unclaimed-binary-content-on-its-content-and-write-chunks-in-packs)
5. [`modules`] [`workspace-analyze`] [`fingerprint`] Stream a binary file through the status reading instead of holding it. [See details](dev/CHANGELOG-DETAILS.md#5-modules-workspace-analyze-fingerprint-stream-a-binary-file-through-the-status-reading-instead-of-holding-it)

## Feature

1. **\[BREAKING!\]** [`app`] [`rola`] [`tree`] Version a directory as one tree. [See details](dev/CHANGELOG-DETAILS.md#1-breaking-app-rola-tree-version-a-directory-as-one-tree)
2. **\[BREAKING!\]** [`app`] [`RorolalaDesktop`] [`windows`] Own and restore floating dock windows. [See details](dev/CHANGELOG-DETAILS.md#2-breaking-app-rorolaladesktop-windows-own-and-restore-floating-dock-windows)
3. [`app`] [`RorolalaDesktop`] [`menu`] Fold the menu bar into a button. [See details](dev/CHANGELOG-DETAILS.md#3-app-rorolaladesktop-menu-fold-the-menu-bar-into-a-button)
4. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`colour`] Mark folders with a colour from a plugin rule. [See details](dev/CHANGELOG-DETAILS.md#4-app-rorolaladesktop-plugins-file-system-colour-mark-folders-with-a-colour-from-a-plugin-rule)
5. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`hides`] Give each hide rule shown, gray and hidden states. [See details](dev/CHANGELOG-DETAILS.md#5-app-rorolaladesktop-plugins-file-system-hides-give-each-hide-rule-shown-gray-and-hidden-states)
6. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`rorolala-vcs`] Offer a tree as a kind of folder a listing can make. [See details](dev/CHANGELOG-DETAILS.md#6-app-rorolaladesktop-plugins-file-system-rorolala-vcs-offer-a-tree-as-a-kind-of-folder-a-listing-can-make)
7. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`title`] Name a directory and its address relative to the base. [See details](dev/CHANGELOG-DETAILS.md#7-app-rorolaladesktop-plugins-file-system-title-name-a-directory-and-its-address-relative-to-the-base)
8. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Choose several entries in the file tree. [See details](dev/CHANGELOG-DETAILS.md#8-app-rorolaladesktop-plugins-file-system-tree-choose-several-entries-in-the-file-tree)
9. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Force a track from the Version Graph with Shift (#7). [See details](dev/CHANGELOG-DETAILS.md#9-app-rorolaladesktop-plugins-rorolala-vcs-force-a-track-from-the-version-graph-with-shift-7)
10. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] [`columns`] Show a tree's status and staging in a directory listing. [See details](dev/CHANGELOG-DETAILS.md#10-app-rorolaladesktop-plugins-rorolala-vcs-columns-show-a-trees-status-and-staging-in-a-directory-listing)
11. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] [`version-graph`] Follow a rola-tree's history in the Version Graph. [See details](dev/CHANGELOG-DETAILS.md#11-app-rorolaladesktop-plugins-rorolala-vcs-version-graph-follow-a-rola-trees-history-in-the-version-graph)
12. [`app`] [`RorolalaDesktop`] [`preview`] Generate file previews in the background. [See details](dev/CHANGELOG-DETAILS.md#12-app-rorolaladesktop-preview-generate-file-previews-in-the-background)
13. [`app`] [`RorolalaDesktop`] [`theme`] State the program's colours in one semantic palette. [See details](dev/CHANGELOG-DETAILS.md#13-app-rorolaladesktop-theme-state-the-programs-colours-in-one-semantic-palette)
14. [`app`] [`RorolalaDesktopHost`] Report a Desktop run that ends with a crash. [See details](dev/CHANGELOG-DETAILS.md#14-app-rorolaladesktophost-report-a-desktop-run-that-ends-with-a-crash)
15. [`librorolala`] [`tree`] Read a tree structure's entries as JSON across the C ABI. [See details](dev/CHANGELOG-DETAILS.md#15-librorolala-tree-read-a-tree-structures-entries-as-json-across-the-c-abi)
16. [`modules`] [`tree`] Read a directory as one version, with staging, baselines and a three-way merge. [See details](dev/CHANGELOG-DETAILS.md#16-modules-tree-read-a-directory-as-one-version-with-staging-baselines-and-a-three-way-merge)

## Refactor

1. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`colour`] Draw folder colours as lines rather than filled blocks. [See details](dev/CHANGELOG-DETAILS.md#1-app-rorolaladesktop-plugins-file-system-colour-draw-folder-colours-as-lines-rather-than-filled-blocks)
2. [`app`] [`RorolalaDesktop`] [`progress`] Show progress in a host-level stack. [See details](dev/CHANGELOG-DETAILS.md#2-app-rorolaladesktop-progress-show-progress-in-a-host-level-stack)

# 0.2.0 (2026-11-04)

## Feature

1. **\[BREAKING!\]** [`app`] [`rola`] [`layout`] Remove an entry from a Vault's Layout, by the account that holds it. [See details](dev/CHANGELOG-DETAILS.md#1-breaking-app-rola-layout-remove-an-entry-from-a-vaults-layout-by-the-account-that-holds-it)
2. **\[BREAKING!\]** [`app`] [`rola`] [`retrack`] Record a version, or another entry, for a file. [See details](dev/CHANGELOG-DETAILS.md#2-breaking-app-rola-retrack-record-a-version-or-another-entry-for-a-file)
3. **\[BREAKING!\]** [`app`] [`rola`] [`sync`] Submit a new file under the path the work tracks it at. [See details](dev/CHANGELOG-DETAILS.md#3-breaking-app-rola-sync-submit-a-new-file-under-the-path-the-work-tracks-it-at)
4. **\[BREAKING!\]** [`modules`] [`daemon`] [`move-remote-path`] Move a path in a Vault's Layout without holding the entry. [See details](dev/CHANGELOG-DETAILS.md#4-breaking-modules-daemon-move-remote-path-move-a-path-in-a-vaults-layout-without-holding-the-entry)
5. [`app`] [`rola`] [`inv-idx`] List the variants and versions the inverse index holds. [See details](dev/CHANGELOG-DETAILS.md#5-app-rola-inv-idx-list-the-variants-and-versions-the-inverse-index-holds)
6. [`app`] [`rola`] [`layout`] Change or renew the Uuid a Layout path names. [See details](dev/CHANGELOG-DETAILS.md#6-app-rola-layout-change-or-renew-the-uuid-a-layout-path-names)
7. [`app`] [`rola-daemon-gui`] [`windows`] Give the Windows daemon window the dark theme of Windows 10 and later. [See details](dev/CHANGELOG-DETAILS.md#7-app-rola-daemon-gui-windows-give-the-windows-daemon-window-the-dark-theme-of-windows-10-and-later)
8. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Draw the desktop Version Graph as the work's variant graph. [See details](dev/CHANGELOG-DETAILS.md#8-app-rorolaladesktop-plugins-rorolala-vcs-draw-the-desktop-version-graph-as-the-works-variant-graph)
9. [`modules`] [`inverse-index`] Read the inverse index the other way, and rebuild it incrementally. [See details](dev/CHANGELOG-DETAILS.md#9-modules-inverse-index-read-the-inverse-index-the-other-way-and-rebuild-it-incrementally)
10. [`modules`] [`status`] [`variant-graph`] Read the variant graph of a workspace as one JSON document. [See details](dev/CHANGELOG-DETAILS.md#10-modules-status-variant-graph-read-the-variant-graph-of-a-workspace-as-one-json-document)

## Refactor

1. **\[BREAKING!\]** [`app`] [`rola`] [`layout`] Remove the `.rolayout` file format. [See details](dev/CHANGELOG-DETAILS.md#1-breaking-app-rola-layout-remove-the-rolayout-file-format)
2. **\[BREAKING!\]** [`modules`] [`workspace`] Keep one Layout per Workspace. [See details](dev/CHANGELOG-DETAILS.md#2-breaking-modules-workspace-keep-one-layout-per-workspace)

# 0.1.3 (2026-11-01)

## Bugfix

1. [`app`] [`rola`] [`pager`] Never page a run that is not talking to a terminal. [See details](dev/CHANGELOG-DETAILS.md#1-app-rola-pager-never-page-a-run-that-is-not-talking-to-a-terminal)
2. [`app`] [`ROLALauncher`] Wait for the Desktop window before leaving the launcher. [See details](dev/CHANGELOG-DETAILS.md#2-app-rolalauncher-wait-for-the-desktop-window-before-leaving-the-launcher)
3. [`app`] [`RorolalaDesktop`] [`plugins`] Fix renaming a row in the desktop's upstream view. [See details](dev/CHANGELOG-DETAILS.md#3-app-rorolaladesktop-plugins-fix-renaming-a-row-in-the-desktops-upstream-view)
4. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] Squeeze the directory dock's columns instead of cutting them off. [See details](dev/CHANGELOG-DETAILS.md#4-app-rorolaladesktop-plugins-file-system-squeeze-the-directory-docks-columns-instead-of-cutting-them-off)
5. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Fill the Overview's table to the dock rather than to its last row. [See details](dev/CHANGELOG-DETAILS.md#5-app-rorolaladesktop-plugins-rorolala-vcs-fill-the-overviews-table-to-the-dock-rather-than-to-its-last-row)
6. [`utils`] [`cli-theme`] [`rendering`] Draw a three-dash Markdown rule as a rule. [See details](dev/CHANGELOG-DETAILS.md#6-utils-cli-theme-rendering-draw-a-three-dash-markdown-rule-as-a-rule)

## Optimized

1. [`app`] [`ROLALauncher`] Open the launcher before reading what it shows. [See details](dev/CHANGELOG-DETAILS.md#1-app-rolalauncher-open-the-launcher-before-reading-what-it-shows)
2. [`app`] [`RorolalaDesktop`] Show the Desktop window before its plugins are assembled. [See details](dev/CHANGELOG-DETAILS.md#2-app-rorolaladesktop-show-the-desktop-window-before-its-plugins-are-assembled)

## Feature

1. **\[BREAKING!\]** [`app`] [`RorolalaDesktop`] [`docking`] Show which dock the user is working in. [See details](dev/CHANGELOG-DETAILS.md#1-breaking-app-rorolaladesktop-docking-show-which-dock-the-user-is-working-in)
2. [`app`] [`rola`] [`desktop`] Ask the Desktop window to say when it is up. [See details](dev/CHANGELOG-DETAILS.md#2-app-rola-desktop-ask-the-desktop-window-to-say-when-it-is-up)
3. [`app`] [`rola`] [`docs`] Read and search the built-in book from the command line. [See details](dev/CHANGELOG-DETAILS.md#3-app-rola-docs-read-and-search-the-built-in-book-from-the-command-line)
4. [`app`] [`RorolalaDesktop`] [`docking`] Mark a dock by its picture in the Window menu and in its tab. [See details](dev/CHANGELOG-DETAILS.md#4-app-rorolaladesktop-docking-mark-a-dock-by-its-picture-in-the-window-menu-and-in-its-tab)
5. [`app`] [`RorolalaDesktop`] [`docking`] Name a dock by the place it is looking at. [See details](dev/CHANGELOG-DETAILS.md#5-app-rorolaladesktop-docking-name-a-dock-by-the-place-it-is-looking-at)
6. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] Copy a cell's words from a directory listing's menu. [See details](dev/CHANGELOG-DETAILS.md#6-app-rorolaladesktop-plugins-file-system-copy-a-cells-words-from-a-directory-listings-menu)
7. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Show a path from an Overview row in its folder, with the version graph following it. [See details](dev/CHANGELOG-DETAILS.md#7-app-rorolaladesktop-plugins-rorolala-vcs-show-a-path-from-an-overview-row-in-its-folder-with-the-version-graph-following-it)
8. [`modules`] [`book-search`] Search the built-in book by word, with the matched fragments. [See details](dev/CHANGELOG-DETAILS.md#8-modules-book-search-search-the-built-in-book-by-word-with-the-matched-fragments)
9. [`utils`] [`cli-theme`] [`rendering`] Lay out a Markdown document for reading. [See details](dev/CHANGELOG-DETAILS.md#9-utils-cli-theme-rendering-lay-out-a-markdown-document-for-reading)
10. [`utils`] [`cli-theme`] [`table`] A full-width listing table for the command line. [See details](dev/CHANGELOG-DETAILS.md#10-utils-cli-theme-table-a-full-width-listing-table-for-the-command-line)
11. [`utils`] [`cli-theme`] [`theme`] Choose the Pretty theme where the terminal can be detected. [See details](dev/CHANGELOG-DETAILS.md#11-utils-cli-theme-theme-choose-the-pretty-theme-where-the-terminal-can-be-detected)

## Refactor

1. [`utils`] [`cli-theme`] [`style`] Keep the Pretty line prefixes the same as the Simple ones. [See details](dev/CHANGELOG-DETAILS.md#1-utils-cli-theme-style-keep-the-pretty-line-prefixes-the-same-as-the-simple-ones)

# 0.1.2 (2026-10-29)

## Optimized

1. **\[BREAKING!\]** [`modules`] Carry a sync's bytes as bytes all the way down, from the packet to the pack. [See details](dev/CHANGELOG-DETAILS.md#1-breaking-modules-carry-a-syncs-bytes-as-bytes-all-the-way-down-from-the-packet-to-the-pack)

## Feature

1. [`app`] [`rola`] Read, select and fetch the work by the managed ignore rules. [See details](dev/CHANGELOG-DETAILS.md#1-app-rola-read-select-and-fetch-the-work-by-the-managed-ignore-rules)
2. [`app`] [`rola`] Speak every action to a Vault behind SSH, one ssh per action. [See details](dev/CHANGELOG-DETAILS.md#2-app-rola-speak-every-action-to-a-vault-behind-ssh-one-ssh-per-action)
3. [`app`] [`ROLALauncher`] Read the scp spelling of a Vault address in the window that creates a Workspace. [See details](dev/CHANGELOG-DETAILS.md#3-app-rolalauncher-read-the-scp-spelling-of-a-vault-address-in-the-window-that-creates-a-workspace)
4. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Hide what a Workspace's ignore rules govern in the browser. [See details](dev/CHANGELOG-DETAILS.md#4-app-rorolaladesktop-plugins-rorolala-vcs-hide-what-a-workspaces-ignore-rules-govern-in-the-browser)
5. [`modules`] Manage ignore rules in one directory, and let a Vault hand rules down. [See details](dev/CHANGELOG-DETAILS.md#5-modules-manage-ignore-rules-in-one-directory-and-let-a-vault-hand-rules-down)
6. [`modules`] Reach a Vault over an SSH session, with no port bound at either end. [See details](dev/CHANGELOG-DETAILS.md#6-modules-reach-a-vault-over-an-ssh-session-with-no-port-bound-at-either-end)

# 0.1.1 (2026-10-28)

## Bugfix

1. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Keep a tree's scroll and chosen step when the files change under it. [See details](dev/CHANGELOG-DETAILS.md#1-app-rorolaladesktop-plugins-file-system-tree-keep-a-trees-scroll-and-chosen-step-when-the-files-change-under-it)
2. [`modules`] [`layout`] Read a shard's snapshot and its log under one lock, and fold a log that has outgrown them. [See details](dev/CHANGELOG-DETAILS.md#2-modules-layout-read-a-shards-snapshot-and-its-log-under-one-lock-and-fold-a-log-that-has-outgrown-them)
3. [`modules`] [`tree-analyze`] [`cache`] Keep a shared analysis cache whole while another reading writes it. [See details](dev/CHANGELOG-DETAILS.md#3-modules-tree-analyze-cache-keep-a-shared-analysis-cache-whole-while-another-reading-writes-it)

## Optimized

1. **\[BREAKING!\]** [`modules`] [`daemon`] Bring the chain under a version from a Vault in one exchange. [See details](dev/CHANGELOG-DETAILS.md#1-breaking-modules-daemon-bring-the-chain-under-a-version-from-a-vault-in-one-exchange)
2. **\[BREAKING!\]** [`modules`] [`daemon`] Open the Vault's Layout once per action and write each run as one batch. [See details](dev/CHANGELOG-DETAILS.md#2-breaking-modules-daemon-open-the-vaults-layout-once-per-action-and-write-each-run-as-one-batch)
3. **\[BREAKING!\]** [`modules`] [`daemon`] [`fetch-layout`] Carry the Vault's Layout as the whole it holds, and pass over a fetch whose copy is already it. [See details](dev/CHANGELOG-DETAILS.md#3-breaking-modules-daemon-fetch-layout-carry-the-vaults-layout-as-the-whole-it-holds-and-pass-over-a-fetch-whose-copy-is-already-it)
4. [`app`] [`rola`] Write a run's ownership changes into the fetched copy once. [See details](dev/CHANGELOG-DETAILS.md#4-app-rola-write-a-runs-ownership-changes-into-the-fetched-copy-once)
5. [`app`] [`rola`] [`retrack`] Read a retrack's chain in one exchange rather than one per object. [See details](dev/CHANGELOG-DETAILS.md#5-app-rola-retrack-read-a-retracks-chain-in-one-exchange-rather-than-one-per-object)
6. [`app`] [`rola`] [`track`] Pack an index with the objects a recording made, and gather the packs when due. [See details](dev/CHANGELOG-DETAILS.md#6-app-rola-track-pack-an-index-with-the-objects-a-recording-made-and-gather-the-packs-when-due)
7. [`modules`] [`daemon`] Resolve a Vault once for as long as its files say the same thing. [See details](dev/CHANGELOG-DETAILS.md#7-modules-daemon-resolve-a-vault-once-for-as-long-as-its-files-say-the-same-thing)
8. [`modules`] [`daemon`] Turn off Nagle on both ends of a session's connections. [See details](dev/CHANGELOG-DETAILS.md#8-modules-daemon-turn-off-nagle-on-both-ends-of-a-sessions-connections)
9. [`modules`] [`daemon`] [`sync-all`] Stop reading the store twice on each side of a sync. [See details](dev/CHANGELOG-DETAILS.md#9-modules-daemon-sync-all-stop-reading-the-store-twice-on-each-side-of-a-sync)
10. [`modules`] [`daemon`] [`sync-index`] Carry an index sync as packets and write it as one pack. [See details](dev/CHANGELOG-DETAILS.md#10-modules-daemon-sync-index-carry-an-index-sync-as-packets-and-write-it-as-one-pack)
11. [`modules`] [`protocol`] Write a small value as one record on the session. [See details](dev/CHANGELOG-DETAILS.md#11-modules-protocol-write-a-small-value-as-one-record-on-the-session)
12. [`modules`] [`vcs`] [`index`] Keep a pack's index in memory, and answer presence for a batch from one reading. [See details](dev/CHANGELOG-DETAILS.md#12-modules-vcs-index-keep-a-packs-index-in-memory-and-answer-presence-for-a-batch-from-one-reading)
13. [`modules`] [`vcs`] [`index`] Write a run of index objects as one pack. [See details](dev/CHANGELOG-DETAILS.md#13-modules-vcs-index-write-a-run-of-index-objects-as-one-pack)

## Feature

1. **\[BREAKING!\]** [`app`] [`RorolalaDesktop`] [`docking`] Replace the dock area's five fixed regions with a recursive layout tree. [See details](dev/CHANGELOG-DETAILS.md#1-breaking-app-rorolaladesktop-docking-replace-the-dock-areas-five-fixed-regions-with-a-recursive-layout-tree)
2. [`app`] [`git-rola`] Add git rola, the git support program. [See details](dev/CHANGELOG-DETAILS.md#2-app-git-rola-add-git-rola-the-git-support-program)
3. [`app`] [`rola-daemon-gui`] Add the GTK 4 window over the daemon on Linux. [See details](dev/CHANGELOG-DETAILS.md#3-app-rola-daemon-gui-add-the-gtk-4-window-over-the-daemon-on-linux)
4. [`dev`] [`rola-build`] [`publish`] Package the Linux hand-off as deb, rpm, Arch and AppImage. [See details](dev/CHANGELOG-DETAILS.md#4-dev-rola-build-publish-package-the-linux-hand-off-as-deb-rpm-arch-and-appimage)
5. [`modules`] [`define-format`] Add rola-define-format, the rola.lock format. [See details](dev/CHANGELOG-DETAILS.md#5-modules-define-format-add-rola-define-format-the-rolalock-format)
6. [`modules`] [`git-support`] Add rola-git-support for project files and the work tree. [See details](dev/CHANGELOG-DETAILS.md#6-modules-git-support-add-rola-git-support-for-project-files-and-the-work-tree)

# 0.1.0 (2026-10-26)

## Feature

1. [`new`] Add the first usable release of the Rorolala VCS suite. [See details](dev/CHANGELOG-DETAILS.md#1-new-add-the-first-usable-release-of-the-rorolala-vcs-suite)
