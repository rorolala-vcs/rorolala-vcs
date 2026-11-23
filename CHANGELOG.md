# CHANGELOG

This document records all Changelog entries for Rorolala VCS starting from 0.1.0, managed by the `cargo changelog` command built into this project.

# 0.4.0 (Unreleased)

## Bugfix

1. [`app`] [`rola`] [`retrack`] Answer a retrack with no earlier version instead of panicking. [See details](dev/CHANGELOG-DETAILS.md#1-app-rola-retrack-answer-a-retrack-with-no-earlier-version-instead-of-panicking)
2. [`app`] [`rola`] [`tree`] Read a short hash in the tree commands that name a version. [See details](dev/CHANGELOG-DETAILS.md#2-app-rola-tree-read-a-short-hash-in-the-tree-commands-that-name-a-version)
3. [`app`] [`RorolalaDesktop`] [`docking`] Give a dock's header strip back when the dock leaves its group. [See details](dev/CHANGELOG-DETAILS.md#3-app-rorolaladesktop-docking-give-a-docks-header-strip-back-when-the-dock-leaves-its-group)
4. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] Ask a row again when the answers behind it were let go of. [See details](dev/CHANGELOG-DETAILS.md#4-app-rorolaladesktop-plugins-file-system-ask-a-row-again-when-the-answers-behind-it-were-let-go-of)
5. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] Stand a place's tile on the plate a file's stands on. [See details](dev/CHANGELOG-DETAILS.md#5-app-rorolaladesktop-plugins-file-system-stand-a-places-tile-on-the-plate-a-files-stands-on)
6. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`icons`] Give a tile the room its badges stand in. [See details](dev/CHANGELOG-DETAILS.md#6-app-rorolaladesktop-plugins-file-system-icons-give-a-tile-the-room-its-badges-stand-in)
7. [`librorolala`] [`write`] [`sync`] Write a tree's work when a sync pulls its version. [See details](dev/CHANGELOG-DETAILS.md#7-librorolala-write-sync-write-a-trees-work-when-a-sync-pulls-its-version)
8. [`librorolala`] [`write`] [`track`] Refuse a tree another account holds, as a file is refused. [See details](dev/CHANGELOG-DETAILS.md#8-librorolala-write-track-refuse-a-tree-another-account-holds-as-a-file-is-refused)
9. [`modules`] [`rola-sharp`] Locate accounts and members without a Vault. [See details](dev/CHANGELOG-DETAILS.md#9-modules-rola-sharp-locate-accounts-and-members-without-a-vault)

## Optimized

1. **\[BREAKING!\]** [`app`] [`RorolalaDesktop`] Stop reading and drawing every view when a window comes back. [See details](dev/CHANGELOG-DETAILS.md#1-breaking-app-rorolaladesktop-stop-reading-and-drawing-every-view-when-a-window-comes-back)
2. **\[BREAKING!\]** [`modules`] [`daemon`] [`sync_all`] Name the window storage sync-all is to carry with. [See details](dev/CHANGELOG-DETAILS.md#2-breaking-modules-daemon-sync_all-name-the-window-storage-sync-all-is-to-carry-with)
3. [`app`] [`RorolalaDesktop`] Draw a large directory, the Version Graph and the terminal with less work. [See details](dev/CHANGELOG-DETAILS.md#3-app-rorolaladesktop-draw-a-large-directory-the-version-graph-and-the-terminal-with-less-work)
4. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] Redraw a dock once for a change instead of once per event. [See details](dev/CHANGELOG-DETAILS.md#4-app-rorolaladesktop-plugins-file-system-redraw-a-dock-once-for-a-change-instead-of-once-per-event)
5. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Leave the views alone when a background fetch changes nothing. [See details](dev/CHANGELOG-DETAILS.md#5-app-rorolaladesktop-plugins-rorolala-vcs-leave-the-views-alone-when-a-background-fetch-changes-nothing)
6. [`modules`] [`daemon`] [`sync_storage`] Read a content's chunks with the window in flight again. [See details](dev/CHANGELOG-DETAILS.md#6-modules-daemon-sync_storage-read-a-contents-chunks-with-the-window-in-flight-again)
7. [`modules`] [`storage`] [`content`] Publish a content into packs instead of a file apiece. [See details](dev/CHANGELOG-DETAILS.md#7-modules-storage-content-publish-a-content-into-packs-instead-of-a-file-apiece)

## Feature

1. [`app`] [`rola`] Draw the unjoined variants in the chain behind --show-unjoined-variants. [See details](dev/CHANGELOG-DETAILS.md#1-app-rola-draw-the-unjoined-variants-in-the-chain-behind---show-unjoined-variants)
2. [`app`] [`rola`] Draw the version chain as one row per variant. [See details](dev/CHANGELOG-DETAILS.md#2-app-rola-draw-the-version-chain-as-one-row-per-variant)
3. [`app`] [`rola`] Stop a running write with Ctrl+C. [See details](dev/CHANGELOG-DETAILS.md#3-app-rola-stop-a-running-write-with-ctrlc)
4. [`app`] [`rola`] [`fetch`] Fetch the content of entries, variants and versions with rola fetch --with. [See details](dev/CHANGELOG-DETAILS.md#4-app-rola-fetch-fetch-the-content-of-entries-variants-and-versions-with-rola-fetch---with)
5. [`app`] [`rola`] [`fsck`] Add fsck to check that the store holds together. [See details](dev/CHANGELOG-DETAILS.md#5-app-rola-fsck-add-fsck-to-check-that-the-store-holds-together)
6. [`app`] [`rola`] [`status`] Show how many entries are ahead, behind or apart in status. [See details](dev/CHANGELOG-DETAILS.md#6-app-rola-status-show-how-many-entries-are-ahead-behind-or-apart-in-status)
7. [`app`] [`rola`] [`status`] Tell a change nobody holds from one another account holds. [See details](dev/CHANGELOG-DETAILS.md#7-app-rola-status-tell-a-change-nobody-holds-from-one-another-account-holds)
8. [`app`] [`rola-daemon-gui`] Start the daemon when the window is asked to run. [See details](dev/CHANGELOG-DETAILS.md#8-app-rola-daemon-gui-start-the-daemon-when-the-window-is-asked-to-run)
9. [`app`] [`ROLALauncher`] [`vaults`] Open the daemon's window from an address's menu. [See details](dev/CHANGELOG-DETAILS.md#9-app-rolalauncher-vaults-open-the-daemons-window-from-an-addresss-menu)
10. [`app`] [`RorolalaDesktop`] [`docking`] Dock a panel against the window's own edge. [See details](dev/CHANGELOG-DETAILS.md#10-app-rorolaladesktop-docking-dock-a-panel-against-the-windows-own-edge)
11. [`app`] [`RorolalaDesktop`] [`font`] Draw the desktop programs in Noto Sans. [See details](dev/CHANGELOG-DETAILS.md#11-app-rorolaladesktop-font-draw-the-desktop-programs-in-noto-sans)
12. [`app`] [`RorolalaDesktop`] [`menu`] Show the layout presets as tabs in the title row. [See details](dev/CHANGELOG-DETAILS.md#12-app-rorolaladesktop-menu-show-the-layout-presets-as-tabs-in-the-title-row)
13. [`app`] [`RorolalaDesktop`] [`plugins`] Show entries that are not on disk as pending, in progress or lost. [See details](dev/CHANGELOG-DETAILS.md#13-app-rorolaladesktop-plugins-show-entries-that-are-not-on-disk-as-pending-in-progress-or-lost)
14. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] Draw file icons on a plate with solid corner badges. [See details](dev/CHANGELOG-DETAILS.md#14-app-rorolaladesktop-plugins-file-system-draw-file-icons-on-a-plate-with-solid-corner-badges)
15. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] Draw the file listing with the Adwaita icon set by default. [See details](dev/CHANGELOG-DETAILS.md#15-app-rorolaladesktop-plugins-file-system-draw-the-file-listing-with-the-adwaita-icon-set-by-default)
16. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] Round a thumbnail's corners and keep its type at the corner. [See details](dev/CHANGELOG-DETAILS.md#16-app-rorolaladesktop-plugins-file-system-round-a-thumbnails-corners-and-keep-its-type-at-the-corner)
17. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] Type a name in a bubble under its row. [See details](dev/CHANGELOG-DETAILS.md#17-app-rorolaladesktop-plugins-file-system-type-a-name-in-a-bubble-under-its-row)
18. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] Wash the rows a colour reaches and tint the icon it is given to. [See details](dev/CHANGELOG-DETAILS.md#18-app-rorolaladesktop-plugins-file-system-wash-the-rows-a-colour-reaches-and-tint-the-icon-it-is-given-to)
19. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] Watch the directories on screen and refresh only what a change reached. [See details](dev/CHANGELOG-DETAILS.md#19-app-rorolaladesktop-plugins-file-system-watch-the-directories-on-screen-and-refresh-only-what-a-change-reached)
20. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] Wrap a tile's name under it and let its line grow. [See details](dev/CHANGELOG-DETAILS.md#20-app-rorolaladesktop-plugins-file-system-wrap-a-tiles-name-under-it-and-let-its-line-grow)
21. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] [`version-graph`] Fetch a version's content from the Version Graph's node menu. [See details](dev/CHANGELOG-DETAILS.md#21-app-rorolaladesktop-plugins-rorolala-vcs-version-graph-fetch-a-versions-content-from-the-version-graphs-node-menu)
22. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] [`version-graph`] Show the whole Version Graph again from its corner. [See details](dev/CHANGELOG-DETAILS.md#22-app-rorolaladesktop-plugins-rorolala-vcs-version-graph-show-the-whole-version-graph-again-from-its-corner)
23. [`app`] [`RorolalaDesktop`] [`ROLALauncher`] [`theme`] Follow the look's file while the other program is open. [See details](dev/CHANGELOG-DETAILS.md#23-app-rorolaladesktop-rolalauncher-theme-follow-the-looks-file-while-the-other-program-is-open)
24. [`app`] [`RorolalaDesktop`] [`theme`] State the light look's semantic colours in the bright set. [See details](dev/CHANGELOG-DETAILS.md#24-app-rorolaladesktop-theme-state-the-light-looks-semantic-colours-in-the-bright-set)
25. [`librorolala`] [`write`] Export the content fetch as rola_write_fetch_with. [See details](dev/CHANGELOG-DETAILS.md#25-librorolala-write-export-the-content-fetch-as-rola_write_fetch_with)
26. [`librorolala`] [`write`] Move the Layout, storage and index writes into librorolala. [See details](dev/CHANGELOG-DETAILS.md#26-librorolala-write-move-the-layout-storage-and-index-writes-into-librorolala)
27. [`librorolala`] [`write`] [`layout`] Write a fetched Vault copy and fork an entry. [See details](dev/CHANGELOG-DETAILS.md#27-librorolala-write-layout-write-a-fetched-vault-copy-and-fork-an-entry)
28. [`modules`] [`rola-sharp`] Add the C# bindings. [See details](dev/CHANGELOG-DETAILS.md#28-modules-rola-sharp-add-the-c-bindings)
29. [`modules`] [`storage`] Bind a Workspace, a Vault or a git-rola project to separate stores. [See details](dev/CHANGELOG-DETAILS.md#29-modules-storage-bind-a-workspace-a-vault-or-a-git-rola-project-to-separate-stores)
30. [`modules`] [`storage`] [`fsck`] Answer whether a store holds together. [See details](dev/CHANGELOG-DETAILS.md#30-modules-storage-fsck-answer-whether-a-store-holds-together)
31. [`utils`] [`constants`] [`abi`] State the ABI version and pin every transparent layout. [See details](dev/CHANGELOG-DETAILS.md#31-utils-constants-abi-state-the-abi-version-and-pin-every-transparent-layout)

## Refactor

1. **\[BREAKING!\]** [`librororala`] [`ffi`] Stop sending and answering JSON across the C ABI. [See details](dev/CHANGELOG-DETAILS.md#1-breaking-librororala-ffi-stop-sending-and-answering-json-across-the-c-abi)
2. **\[BREAKING!\]** [`modules`] [`daemon`] [`abi`] Type the daemon action requests and answers across the C ABI. [See details](dev/CHANGELOG-DETAILS.md#2-breaking-modules-daemon-abi-type-the-daemon-action-requests-and-answers-across-the-c-abi)
3. **\[BREAKING!\]** [`modules`] [`rola-sharp`] Remove the first-generation C# bindings and keep RolaSharp as the one assembly. [See details](dev/CHANGELOG-DETAILS.md#3-breaking-modules-rola-sharp-remove-the-first-generation-c-bindings-and-keep-rolasharp-as-the-one-assembly)
4. **\[BREAKING!\]** [`utils`] [`location`] [`user-data`] Keep each build's user data under its userdata name. [See details](dev/CHANGELOG-DETAILS.md#4-breaking-utils-location-user-data-keep-each-builds-user-data-under-its-userdata-name)
5. [`app`] [`rola`] Run the write commands through librorolala. [See details](dev/CHANGELOG-DETAILS.md#5-app-rola-run-the-write-commands-through-librorolala)
6. [`app`] [`ROLALauncher`] [`settings`] Center the launcher's setting names and drop their hints. [See details](dev/CHANGELOG-DETAILS.md#6-app-rolalauncher-settings-center-the-launchers-setting-names-and-drop-their-hints)
7. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] Stop drawing a word over an empty folder. [See details](dev/CHANGELOG-DETAILS.md#7-app-rorolaladesktop-plugins-file-system-stop-drawing-a-word-over-an-empty-folder)
8. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Draw the folder tree as a list of places. [See details](dev/CHANGELOG-DETAILS.md#8-app-rorolaladesktop-plugins-file-system-tree-draw-the-folder-tree-as-a-list-of-places)
9. [`app`] [`RorolalaDesktop`] [`theme`] Draw a scrollbar as a quiet pill over the content. [See details](dev/CHANGELOG-DETAILS.md#9-app-rorolaladesktop-theme-draw-a-scrollbar-as-a-quiet-pill-over-the-content)
10. [`app`] [`RorolalaVCSPlugin`] Bring and own entries through the C# writers. [See details](dev/CHANGELOG-DETAILS.md#10-app-rorolalavcsplugin-bring-and-own-entries-through-the-c-writers)
11. [`app`] [`RorolalaVCSPlugin`] Read the ignore rules, the Workspace and the account from the C# bindings. [See details](dev/CHANGELOG-DETAILS.md#11-app-rorolalavcsplugin-read-the-ignore-rules-the-workspace-and-the-account-from-the-c-bindings)
12. [`app`] [`RorolalaVCSPlugin`] Run track, align and retrack through the C# writers. [See details](dev/CHANGELOG-DETAILS.md#12-app-rorolalavcsplugin-run-track-align-and-retrack-through-the-c-writers)
13. [`app`] [`RorolalaVCSPlugin`] [`graph`] Read the version graph from the C# bindings. [See details](dev/CHANGELOG-DETAILS.md#13-app-rorolalavcsplugin-graph-read-the-version-graph-from-the-c-bindings)
14. [`app`] [`RorolalaVCSPlugin`] [`layout`] Move a path through the C# Layout writer. [See details](dev/CHANGELOG-DETAILS.md#14-app-rorolalavcsplugin-layout-move-a-path-through-the-c-layout-writer)
15. [`app`] [`RorolalaVCSPlugin`] [`overview`] Read the overview from the C# bindings. [See details](dev/CHANGELOG-DETAILS.md#15-app-rorolalavcsplugin-overview-read-the-overview-from-the-c-bindings)
16. [`app`] [`RorolalaVCSPlugin`] [`sync`] Run sync through the C# writer. [See details](dev/CHANGELOG-DETAILS.md#16-app-rorolalavcsplugin-sync-run-sync-through-the-c-writer)
17. [`app`] [`RorolalaVCSPlugin`] [`tree`] Read a tree from the C# bindings. [See details](dev/CHANGELOG-DETAILS.md#17-app-rorolalavcsplugin-tree-read-a-tree-from-the-c-bindings)
18. [`app`] [`TerminalPlugin`] Run the terminal's pty through the C# bindings. [See details](dev/CHANGELOG-DETAILS.md#18-app-terminalplugin-run-the-terminals-pty-through-the-c-bindings)
19. [`librorolala`] Resolve accounts and Vaults from one implementation. [See details](dev/CHANGELOG-DETAILS.md#19-librorolala-resolve-accounts-and-vaults-from-one-implementation)
20. [`librorolala`] [`vcs`] Drop the chain's terminal drawing. [See details](dev/CHANGELOG-DETAILS.md#20-librorolala-vcs-drop-the-chains-terminal-drawing)
21. [`librororala`] [`abi`] Name every C ABI export after one rule. [See details](dev/CHANGELOG-DETAILS.md#21-librororala-abi-name-every-c-abi-export-after-one-rule)

# 0.3.0 (2026-11-17)

## Bugfix

1. [`app`] [`rola`] [`sync`] [`tree`] Carry a tree's whole chain when a sync uploads or a checkin brings it in. [See details](dev/CHANGELOG-DETAILS.md#1-app-rola-sync-tree-carry-a-trees-whole-chain-when-a-sync-uploads-or-a-checkin-brings-it-in)
2. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`icons`] Draw badge pictures at whole pixels and without a circular clip. [See details](dev/CHANGELOG-DETAILS.md#2-app-rorolaladesktop-plugins-file-system-icons-draw-badge-pictures-at-whole-pixels-and-without-a-circular-clip)
3. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Draw a chosen tree row once and centre its badges. [See details](dev/CHANGELOG-DETAILS.md#3-app-rorolaladesktop-plugins-file-system-tree-draw-a-chosen-tree-row-once-and-centre-its-badges)
4. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Keep the local pill on the draft while a file is edited. [See details](dev/CHANGELOG-DETAILS.md#4-app-rorolaladesktop-plugins-rorolala-vcs-keep-the-local-pill-on-the-draft-while-a-file-is-edited)
5. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Keep the message field's pointer and row steady. [See details](dev/CHANGELOG-DETAILS.md#5-app-rorolaladesktop-plugins-rorolala-vcs-keep-the-message-fields-pointer-and-row-steady)
6. [`modules`] [`storage`] [`pack`] Forget a pack's listing when its last index goes. [See details](dev/CHANGELOG-DETAILS.md#6-modules-storage-pack-forget-a-packs-listing-when-its-last-index-goes)

## Optimized

1. [`modules`] [`chunking`] [`text`] Stop a text cut at the bound when a line runs past it. [See details](dev/CHANGELOG-DETAILS.md#1-modules-chunking-text-stop-a-text-cut-at-the-bound-when-a-line-runs-past-it)
2. [`modules`] [`inverse-index`] [`table`] Build the inverse index in the buffer it is written from. [See details](dev/CHANGELOG-DETAILS.md#2-modules-inverse-index-table-build-the-inverse-index-in-the-buffer-it-is-written-from)
3. [`modules`] [`protocol`] [`frame`] Read a carried frame into the buffer it is being put together in. [See details](dev/CHANGELOG-DETAILS.md#3-modules-protocol-frame-read-a-carried-frame-into-the-buffer-it-is-being-put-together-in)
4. [`modules`] [`storage`] Cut unclaimed binary content on its content, and write chunks in packs. [See details](dev/CHANGELOG-DETAILS.md#4-modules-storage-cut-unclaimed-binary-content-on-its-content-and-write-chunks-in-packs)
5. [`modules`] [`workspace-analyze`] [`fingerprint`] Stream a binary file through the status reading instead of holding it. [See details](dev/CHANGELOG-DETAILS.md#5-modules-workspace-analyze-fingerprint-stream-a-binary-file-through-the-status-reading-instead-of-holding-it)

## Feature

1. **\[BREAKING!\]** [`app`] [`rola`] [`tree`] Version a directory as one tree. [See details](dev/CHANGELOG-DETAILS.md#1-breaking-app-rola-tree-version-a-directory-as-one-tree)
2. **\[BREAKING!\]** [`app`] [`RorolalaDesktop`] Check ownership before opening a file from the Desktop. [See details](dev/CHANGELOG-DETAILS.md#2-breaking-app-rorolaladesktop-check-ownership-before-opening-a-file-from-the-desktop)
3. [`app`] [`cli`] Draw the help page as a heading, a marked usage line and underlined flags. [See details](dev/CHANGELOG-DETAILS.md#3-app-cli-draw-the-help-page-as-a-heading-a-marked-usage-line-and-underlined-flags)
4. [`app`] [`cli`] [`RorolalaDesktop`] [`plugins`] [`tree`] Restore a tree's file to its baseline or its staging area. [See details](dev/CHANGELOG-DETAILS.md#4-app-cli-rorolaladesktop-plugins-tree-restore-a-trees-file-to-its-baseline-or-its-staging-area)
5. [`app`] [`cli`] [`RorolalaDesktop`] [`plugins`] [`tree`] Retrack a tree to a version, an entry, or a fork. [See details](dev/CHANGELOG-DETAILS.md#5-app-cli-rorolaladesktop-plugins-tree-retrack-a-tree-to-a-version-an-entry-or-a-fork)
6. [`app`] [`desktop`] [`launcher`] [`rola`] [`git-rola`] Open a git-rola project's Vaults in a view and record a move in its two files. [See details](dev/CHANGELOG-DETAILS.md#6-app-desktop-launcher-rola-git-rola-open-a-git-rola-projects-vaults-in-a-view-and-record-a-move-in-its-two-files)
7. [`app`] [`git-rola`] List a lock's paths and sign a Vault's in with bring. [See details](dev/CHANGELOG-DETAILS.md#7-app-git-rola-list-a-locks-paths-and-sign-a-vaults-in-with-bring)
8. [`app`] [`git-rola`] Place a tree material as the directory its structure names. [See details](dev/CHANGELOG-DETAILS.md#8-app-git-rola-place-a-tree-material-as-the-directory-its-structure-names)
9. [`app`] [`rola`] [`vcs-index`] Read whether a version's content is here, and ask which Vault holds it. [See details](dev/CHANGELOG-DETAILS.md#9-app-rola-vcs-index-read-whether-a-versions-content-is-here-and-ask-which-vault-holds-it)
10. [`app`] [`RorolalaDesktop`] [`docking`] Switch the arrangement from a layout preset. [See details](dev/CHANGELOG-DETAILS.md#10-app-rorolaladesktop-docking-switch-the-arrangement-from-a-layout-preset)
11. [`app`] [`RorolalaDesktop`] [`menu`] Fold the menu bar into a button. [See details](dev/CHANGELOG-DETAILS.md#11-app-rorolaladesktop-menu-fold-the-menu-bar-into-a-button)
12. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`colour`] Mark folders with a colour from a plugin rule. [See details](dev/CHANGELOG-DETAILS.md#12-app-rorolaladesktop-plugins-file-system-colour-mark-folders-with-a-colour-from-a-plugin-rule)
13. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`hides`] Give each hide rule shown, gray and hidden states. [See details](dev/CHANGELOG-DETAILS.md#13-app-rorolaladesktop-plugins-file-system-hides-give-each-hide-rule-shown-gray-and-hidden-states)
14. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`icons`] Explain every file badge on hover. [See details](dev/CHANGELOG-DETAILS.md#14-app-rorolaladesktop-plugins-file-system-icons-explain-every-file-badge-on-hover)
15. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`rorolala-vcs`] Offer a tree as a kind of folder a listing can make. [See details](dev/CHANGELOG-DETAILS.md#15-app-rorolaladesktop-plugins-file-system-rorolala-vcs-offer-a-tree-as-a-kind-of-folder-a-listing-can-make)
16. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`title`] Name a directory and its address relative to the base. [See details](dev/CHANGELOG-DETAILS.md#16-app-rorolaladesktop-plugins-file-system-title-name-a-directory-and-its-address-relative-to-the-base)
17. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Choose several entries in the file tree. [See details](dev/CHANGELOG-DETAILS.md#17-app-rorolaladesktop-plugins-file-system-tree-choose-several-entries-in-the-file-tree)
18. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Draw a tree's levels as guides and its status on its row. [See details](dev/CHANGELOG-DETAILS.md#18-app-rorolaladesktop-plugins-file-system-tree-draw-a-trees-levels-as-guides-and-its-status-on-its-row)
19. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Give the folder tree the listing's delete, rename and new-folder keys. [See details](dev/CHANGELOG-DETAILS.md#19-app-rorolaladesktop-plugins-file-system-tree-give-the-folder-tree-the-listings-delete-rename-and-new-folder-keys)
20. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Force a track from the Version Graph with Shift (#7). [See details](dev/CHANGELOG-DETAILS.md#20-app-rorolaladesktop-plugins-rorolala-vcs-force-a-track-from-the-version-graph-with-shift-7)
21. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Manage a tree's staging area and description in a Tree dock. [See details](dev/CHANGELOG-DETAILS.md#21-app-rorolaladesktop-plugins-rorolala-vcs-manage-a-trees-staging-area-and-description-in-a-tree-dock)
22. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Mark a Version Graph node whose content is stored here. [See details](dev/CHANGELOG-DETAILS.md#22-app-rorolaladesktop-plugins-rorolala-vcs-mark-a-version-graph-node-whose-content-is-stored-here)
23. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Show owners and move ownership in the upstream view. [See details](dev/CHANGELOG-DETAILS.md#23-app-rorolaladesktop-plugins-rorolala-vcs-show-owners-and-move-ownership-in-the-upstream-view)
24. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] [`columns`] Show a tree's status and staging in a directory listing. [See details](dev/CHANGELOG-DETAILS.md#24-app-rorolaladesktop-plugins-rorolala-vcs-columns-show-a-trees-status-and-staging-in-a-directory-listing)
25. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] [`fetch`] Fetch the Vault's Layout from a loop and show its state under the Overview. [See details](dev/CHANGELOG-DETAILS.md#25-app-rorolaladesktop-plugins-rorolala-vcs-fetch-fetch-the-vaults-layout-from-a-loop-and-show-its-state-under-the-overview)
26. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] [`version-graph`] Focus the chosen file's node with F in the Version Graph. [See details](dev/CHANGELOG-DETAILS.md#26-app-rorolaladesktop-plugins-rorolala-vcs-version-graph-focus-the-chosen-files-node-with-f-in-the-version-graph)
27. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] [`version-graph`] Follow a rola-tree's history in the Version Graph. [See details](dev/CHANGELOG-DETAILS.md#27-app-rorolaladesktop-plugins-rorolala-vcs-version-graph-follow-a-rola-trees-history-in-the-version-graph)
28. [`app`] [`RorolalaDesktop`] [`preview`] Generate file previews in the background. [See details](dev/CHANGELOG-DETAILS.md#28-app-rorolaladesktop-preview-generate-file-previews-in-the-background)
29. [`app`] [`RorolalaDesktop`] [`theme`] State the program's colours in one semantic palette. [See details](dev/CHANGELOG-DETAILS.md#29-app-rorolaladesktop-theme-state-the-programs-colours-in-one-semantic-palette)
30. [`app`] [`RorolalaDesktop`] [`window`] Name the program and workspace in the title row. [See details](dev/CHANGELOG-DETAILS.md#30-app-rorolaladesktop-window-name-the-program-and-workspace-in-the-title-row)
31. [`app`] [`RorolalaDesktop`] [`windows`] Own and restore floating dock windows. [See details](dev/CHANGELOG-DETAILS.md#31-app-rorolaladesktop-windows-own-and-restore-floating-dock-windows)
32. [`app`] [`RorolalaDesktopHost`] Report a Desktop run that ends with a crash. [See details](dev/CHANGELOG-DETAILS.md#32-app-rorolaladesktophost-report-a-desktop-run-that-ends-with-a-crash)
33. [`librorolala`] [`ffi`] Read Layouts, configuration, storage and tree marks across the C ABI. [See details](dev/CHANGELOG-DETAILS.md#33-librorolala-ffi-read-layouts-configuration-storage-and-tree-marks-across-the-c-abi)
34. [`librorolala`] [`tree`] Read a tree's entries and status as JSON across the C ABI. [See details](dev/CHANGELOG-DETAILS.md#34-librorolala-tree-read-a-trees-entries-and-status-as-json-across-the-c-abi)
35. [`modules`] [`rola-sharp`] Read Layouts, configuration, tree marks and identities from C#. [See details](dev/CHANGELOG-DETAILS.md#35-modules-rola-sharp-read-layouts-configuration-tree-marks-and-identities-from-c)
36. [`modules`] [`tree`] Read a directory as one version, with staging, baselines and a three-way merge. [See details](dev/CHANGELOG-DETAILS.md#36-modules-tree-read-a-directory-as-one-version-with-staging-baselines-and-a-three-way-merge)

## Refactor

1. **\[BREAKING!\]** [`app`] [`git-rola`] Speak git's language instead of the program's own. [See details](dev/CHANGELOG-DETAILS.md#1-breaking-app-git-rola-speak-gits-language-instead-of-the-programs-own)
2. **\[BREAKING!\]** [`app`] [`rola`] [`bring`] Rename the checkin command to bring. [See details](dev/CHANGELOG-DETAILS.md#2-breaking-app-rola-bring-rename-the-checkin-command-to-bring)
3. **\[BREAKING!\]** [`app`] [`RorolalaDesktop`] [`menus`] Reorganise every context menu into categories. [See details](dev/CHANGELOG-DETAILS.md#3-breaking-app-rorolaladesktop-menus-reorganise-every-context-menu-into-categories)
4. **\[BREAKING!\]** [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Rename the desktop plugin's check-in flow to bring. [See details](dev/CHANGELOG-DETAILS.md#4-breaking-app-rorolaladesktop-plugins-rorolala-vcs-rename-the-desktop-plugins-check-in-flow-to-bring)
5. **\[BREAKING!\]** [`modules`] [`define-format`] Record a material's kind in rola.lock. [See details](dev/CHANGELOG-DETAILS.md#5-breaking-modules-define-format-record-a-materials-kind-in-rolalock)
6. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Draw a file tree row as a three-column grid. [See details](dev/CHANGELOG-DETAILS.md#6-app-rorolaladesktop-plugins-file-system-tree-draw-a-file-tree-row-as-a-three-column-grid)
7. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] [`version-graph`] Draw the Version Graph in the look's own hues. [See details](dev/CHANGELOG-DETAILS.md#7-app-rorolaladesktop-plugins-rorolala-vcs-version-graph-draw-the-version-graph-in-the-looks-own-hues)
8. [`app`] [`RorolalaDesktop`] [`progress`] Show progress in a host-level stack. [See details](dev/CHANGELOG-DETAILS.md#8-app-rorolaladesktop-progress-show-progress-in-a-host-level-stack)
9. [`app`] [`RorolalaDesktop`] [`theme`] Restate the look and dock chrome. [See details](dev/CHANGELOG-DETAILS.md#9-app-rorolaladesktop-theme-restate-the-look-and-dock-chrome)

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
