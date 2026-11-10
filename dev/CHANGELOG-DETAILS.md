# CHANGELOG

This document records all Changelog entries for Rorolala VCS starting from 0.1.0, managed by the `cargo changelog` command built into this project.

# 0.3.0 (Unreleased)

## Bugfix

### 1. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`icons`] Colour a tagged badge's ground from the theme's dark role

A mark worn on a picture had its ground fixed at `#1A1A1A`, so it did not follow the dark theme a reader chose. The ground is now the look's dark role, read inside the badge's dark scope, so the park on a tree's folder and every other tagged badge take the dark the palette in force names while staying dark on the light look.

### 2. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`icons`] Draw badge pictures at whole pixels and without a circular clip

A badge's picture was clipped to a circle and scaled to a fractional size, so a small mark lost its corners and read as a blur. The picture now keeps its square mask, is scaled to a whole number of pixels, and the badge is laid out on whole pixels; the badge's own ground remains the circle. The picture is also reduced once to the size it is drawn at and kept, rather than being reduced by the compositor every time it is shown. A tree row's badges are moved down by a whole pixel rather than a half, so the render transform that centres them no longer resamples their pictures.

### 3. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Draw a chosen tree row once and centre its badges

A chosen tree row is now one rounded wash over the whole step, rather than a wash on the row and a second one around its name and icon. The badges at the row's right are centred on the step, so the room above and below them is the same.

### 4. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Keep a tree row's badges clear of its name

A file tree row is now a three-column grid rather than a dock panel: the name gives way with an ellipsis, the badges keep their width at the row's right, and the row stretches to the tree's width at any panel width. The badges are drawn at twenty pixels, while the entry icon stays at sixteen.

### 5. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Keep a tree's guide layer out of the arrange pass while resizing

Resizing the folder tree rebuilt the guide layer of every row while the rows' header grids were being arranged, which could end the program with a null reference inside the grid's arrange. The guide layer is now refreshed after the layout pass, and a line covered by a tree's coloured line is hidden rather than removed, so a resize changes only what is shown and never the row's children.

### 6. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Keep the local pill on the draft while a file is edited

The Version Graph no longer draws an edited file's local pill on the version its draft is based on
as well as on the draft. While the file has an unrecorded change, its draft is the one card that
names it; the baseline version keeps the entries of the other sides.

### 7. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Keep the message field's pointer and row steady

The message field under the Version Graph is the whole room beside the TRACK button, but the pointer read as text only over the line the message is drawn on: the padding around that line and the empty width after it read as ground, although a press there focused the field. The field now names the text cursor itself, so every part of its room inherits it.

The hint is a text block and took the body type rather than the field's, so it was taller than the line that replaced it and the strip shrank when typing started. The hint now takes the field's own type, so a row showing the hint and a row showing a message are the same height.

## Optimized

### 1. [`modules`] [`chunking`] [`text`] Stop a text cut at the bound when a line runs past it

A text cut was moved on to the end of the line it landed in with no bound, so a file of one enormous line, a minified script or a single-line JSON, had its first cut moved to the end of the file and was stored as a single chunk. The move now stops when the line has more than `max` left to run, and the line is cut through there.

Short lines are unchanged: a chunk still ends at a line end, and may run past `max` by the line it moved over. No chunk is longer than twice `max`.

### 2. [`modules`] [`inverse-index`] [`table`] Build the inverse index in the buffer it is written from

`Table::encode` built the records in a buffer of their own and copied them behind the header, for a table that may be tens of megabytes. The records are now written directly behind the header and the digest is filled in afterwards.

### 3. [`modules`] [`protocol`] [`frame`] Read a carried frame into the buffer it is being put together in

A frame read for a blob carried in pieces allocated a buffer of its own and then copied it into the blob being assembled. `read_frame_into` reads the frame into the tail of the destination, so a large content crossing a sync is copied once rather than twice.

### 4. [`modules`] [`storage`] Cut unclaimed binary content on its content, and write chunks in packs

Binary content no policy claims and no ZIP signature marks was written whole, so an edit to a large PSB, TIFF, EXR, TGA, WAV or FBX binary rewrote the whole file. Content of at least 256 KiB is now cut on its content, with chunks of 16 KiB to 256 KiB, so an edit costs the chunks it fell in.

A chunked write lays its chunks down in a pack rather than as one file each, and a chunked read plans every chunk before reading any of them, so a content of thousands of chunks no longer costs a file, a pack lookup and a pack open per chunk. The manifest is still written loose, and the chunks are published before it is written.

Neither the cut nor its bounds are recorded in the store, so content written before this reads back unchanged. The first save after upgrading re-cuts a large binary once, and later versions share chunks with it.

A pack left empty by a removal is dropped from the store and forgotten from the list of packs at once, rather than waiting for a directory timestamp to change, so a store that has let go of its last pack does not go on naming it.

Reads and writes no longer copy content on the way through: a payload stored as it is is borrowed rather than encoded into a second buffer, an entry is read frame first with its payload into the front of its own buffer, a manifest is decoded in place, and a file's key is hashed as it streams. An extract writes each chunk to the file and hashes it as it goes rather than putting the whole content together in memory, and a chunked write canonicalises the store once rather than once a chunk.

### 5. [`modules`] [`workspace-analyze`] [`fingerprint`] Stream a binary file through the status reading instead of holding it

`rola status` read every changed file whole to fingerprint it, so a two gigabyte `PSD` took two gigabytes of memory. A file is now opened, sniffed on its first 8000 bytes, and hashed as it streams when it reads as binary; only a text file is held whole, because the spans a move is guessed from are cut out of all of it. The tree reading uses the same stream.

## Feature

### 1. **\[BREAKING!\]** [`app`] [`rola`] [`tree`] Version a directory as one tree

`rola tree` is a new namespace for directories that are versions in their own right: `init`, `status`, `add`, `rm`, `desc`, `write-desc`, `cat-desc`, `ls-tree`, `ls-staging-tree` and `diff-stat`. A directory with a `.rola-tree` mark is one unit of work, `rola track` records its staged structure as one version, and it is named and described in the same list as a file: the group message, its own `--file-message` by position, and the same editor sections all work for it, Running `rola status` inside a tree reads the tree itself, while naming a tree draws its version chain the way naming a file does, with a draft above the chain while its work is unrecorded. From outside a tree, its work is one row among the Workspace's structural and content changes, a tree object beside the paths, with trees drawn before the files and the parts that are zero left out. Aligning a tree with `--restore`, and moving its record back with `rola retrack`, put the whole directory back from a version. A tree whose directory was deleted is restored whole as well: which paths are trees is read from the Layout rather than from a mark that is gone, so the directory, its `.rola-tree` skeleton and its baseline are recreated, a path inside the tree names the tree, and a file sitting where the tree belongs is replaced. Naming a tree or anything under it in `rola hold` or `rola giveup` names the tree's one entry, so its ownership moves as one thing. A tree merges with `rola checkin --join <VARIANT> --tree-merge`, which compares two structures over their shared version and leaves `~theirs` copies beside the work for what both sides changed. The C ABI gains `rola_tree_entries_json`.

A tree records the id of the Layout entry it is at in its `.rola-tree` mark, so a directory moved outside Rorolala is found again as one move by `rola status` and `rola layout tree-diff`. A tree with no id to match is read as a loss and a gain, and `rola align --move` connects the two so the next reading pairs them.

Breaking: the Layout snapshot format is version 3 and its records carry a kind byte. A Workspace or Vault written by an earlier version cannot be read by this one, and a Vault written here cannot be read by an earlier one. A sync carries the kind of each entry it creates, so a Vault's Layout records trees as trees, and a path the two sides name as different things is refused rather than transferred.

### 2. **\[BREAKING!\]** [`app`] [`RorolalaDesktop`] [`windows`] Own and restore floating dock windows

A dock pulled into a window of its own is now owned by the main window. Closing the main window closes its children, minimizing it minimizes them with it, and restoring it gives each child back the state it had. The layout file is version 3 and records each floating window's dock, ordinal, own state, bounds and maximized state; version 2 files still load, with no floating windows. A window restored off screen is moved back onto the primary work area.

### 3. [`app`] [`RorolalaDesktop`] [`menu`] Fold the menu bar into a button

The title row no longer carries a menu bar. One button at its left opens the same top-level menus in a flyout, in their old order, with the Window menu still listing the docks and its commands. The button has a tooltip and an accessible name, and a press on it does not move the window.

### 4. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`colour`] Mark folders with a colour from a plugin rule

Entries can now be marked with a colour by a rule a plugin registers. A marked directory gives its colour to everything under it, the nearest marked directory wins, and the file tree draws one block behind the whole subtree, so the colour is one rectangle rather than a wash on each row. A marked file carries its own block. A directory listing draws a wash on the row of each entry the colour reaches, the entries under a marked directory darker than the directory itself, and the Directories dock's bright ground carries a share of the colour while it is inside a marked directory. Icons keep their own colours, except for the park a rola-tree wears: it is drawn in the colour the tree's own id names, in the badge column at a tree row's right and on the folder's corner in a directory listing, so a tree is told from a plain directory before its name is read.

The Rorolala plugin registers the first rule. It marks a rola-tree with the hue of the hash of the id in its `.rola-tree` mark, so the colour travels with the directory and a copy of one tree is the same colour.

### 5. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`hides`] Give each hide rule shown, gray and hidden states

Each hide rule now has three states rather than two. Shown leaves its entries as they are, Gray keeps them in the listing at 55% opacity, and Hidden leaves them out; a click on the rule cycles shown, gray, hidden. The popup lists the rules as Dot Directories, Dot Files, Windows Hidden Files, Git Ignored Files and Rola Ignored Files, and its eye and more button are one run at the toolbar's right, separated from the link by a 1 px rule. The eye shows the entries a hiding rule leaves out as gray without changing the rule's state.

### 6. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`title`] Name a directory and its address relative to the base

A directory dock inside the tree's base is named by its part under that base, so a tab reads `art` or `art/shots` rather than the whole path, and its address bar starts at `.` for the base and adds one step for each directory under it. The detail a tab and a window title show on hover is still the full path, a directory outside the base keeps its full path, and a logical place keeps its address.

### 7. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`tree`] Choose several entries in the file tree

The file tree now supports the same choice as a directory listing: a plain press chooses one row, Ctrl toggles one, Shift takes a visible range, and Ctrl+A chooses every row on screen. A modified press on a directory changes the choice without navigating, while a plain press still opens it. Copy, cut, delete and the context menu act on the whole choice, and a tree read again keeps its choice by path.

### 8. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Force a track from the Version Graph with Shift (#7)

The TRACK button under the Version Graph turns red while Shift is held. A press then records
the chosen changes with `rola track --force`, and a question names the files whose changes a plain
track refuses before anything is written. Without Shift the button records the changes as before.

### 9. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] [`columns`] Show a tree's status and staging in a directory listing

Inside a rola-tree, the Directories dock now shows Status and Staged columns in place of Upstream, Standing and Owner. Status says whether a path is new or changed, and Staged says whether all of it, some of it or none of it is staged. Each answer is also worn on the path's icon: a green plus for new, a yellow pencil for changed, and a full, half or empty circle for the three staging states.

### 10. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] [`version-graph`] Follow a rola-tree's history in the Version Graph

The Version Graph now treats a rola-tree as one versioned entry. Choosing the tree's directory, or any file inside it, centres the graph on the tree's own chain rather than on a path the Layout does not name. Choosing a version's local entry pill opens the tree's parent and selects the tree directory, so the graph and the listing agree on the object being read. The same message field and TRACK button under the graph record the tree when its directory or anything under it is chosen.

A tree's unrecorded work is drawn as a draft above its chain, including changes that are only in the tree's staging area and a staged description with no changed path. The C ABI gains `rola_tree_status_json`, the desktop reads it through `RolaTreeStatus`, and a focus on the Workspace root is accepted as the root tree's entry.

### 11. [`app`] [`RorolalaDesktop`] [`preview`] Generate file previews in the background

A file browser's grid now shows a file's own preview where one can be made, and keeps the type icon where one cannot. The host asks its system source first and the plugins' renderers after it, keeps answers by path, write time and size, cancels a tile's request when the tile leaves the window, and reports a batch on the progress stack. Windows uses the Shell thumbnailer on a dedicated STA thread; Linux reads thumbnails the desktop has already cached.

### 12. [`app`] [`RorolalaDesktopHost`] Report a Desktop run that ends with a crash

`rola desktop` now starts a host beside the Desktop rather than the Desktop itself. The host forwards the line that says the window is up, keeps the Desktop's error stream, and when the run ends with a non-zero code writes the report to a log and opens a window that shows it. A Desktop run that throws also writes the exception's stack to that stream, so the stack survives the window that wrote it. A start that found the Desktop already running, exit code 174, is passed through without a report.

### 13. [`librorolala`] [`tree`] Read a tree structure's entries as JSON across the C ABI

`rola_tree_entries_json` takes a tree structure's bytes as hex and answers with its entries as JSON: each entry's path, kind, mode, storage key and link target. A caller that cannot decode the structure format hands the bytes over instead of keeping a second copy of it, and text that is not hex or bytes that are not a structure answer as `null`, so an answer is always JSON. The string is freed with `free_string`.

### 14. [`modules`] [`tree`] Read a directory as one version, with staging, baselines and a three-way merge

The new `rorolala-tree` crate holds what a tree is: a canonical structure object built from paths, kinds, modes and content keys, with its own binary format and strict decoding. It stages work in a `.rola-tree/staging` store, keeps a baseline structure and a fingerprint cache beside it, reads status by comparing the recorded version, the staged structure and the work, and merges two structures over their shared version path by path. Content moves into the Workspace's store with `publish_from`, which carries manifests and their chunks without rewriting what is already there.

`rola tree reset` puts the staging area back to the recorded version, for the whole tree or only the paths named, and leaves the work untouched; it has no hard or soft form, since `retrack` and `retrack --only-index` move the work and the record. `rola tree desc` opens the same editing file and editor as `rola track`, and stages what the editor wrote only after it ends successfully, so a cancelled edit leaves the staged description as it was. A rules file may sit in any directory of a tree, and the rules apply the way git's nested `.gitignore` files do: a pattern is relative to the file holding it, a deeper file decides after a shallower one, and the rules file itself stays tracked work. A Workspace whose root is a tree keeps that tree at the reserved root path, shown as `.`, and `rola status .` looks its version chain back with a draft above it while its work is unrecorded. A tree's structure names what its version holds, so a sync carries the head version's structure and its description and file content in a second exchange. A Workspace scan treats a directory holding a `.rola-tree` mark as one tree entry rather than the files under it, and the Layout, its records and its snapshot carry a kind so a file and a tree are told apart.

## Refactor

### 1. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] [`colour`] Draw folder colours as lines rather than filled blocks

A marked folder or file is now drawn with a line rather than a fill. The folder tree draws one thick coloured line beside a marked step, through the whole height of its open subtree, with an inner marked step standing at its own deeper indent. A directory list draws the colour as a strip at the row's left edge, and a directory grid draws it as a thick line under the caption. The row and tile fills and the ground tint inside a marked directory are gone, so names and pictures stand on the dock's own ground. A tree's own row also wears the state a file's row does: the green dot of something new or the yellow dot of something changed, read from the tree's status.

### 2. [`app`] [`RorolalaDesktop`] [`progress`] Show progress in a host-level stack

Progress is now the host's rather than the Rorolala plugin's. `IPluginHost.Progress` begins a run that every open window draws in a stack under its title row, and the plugin's commands use it without knowing which window is in front. The Overview no longer draws a panel of its own, and a run begun from any dock appears in the same stack.

# 0.2.0 (2026-11-04)

## Feature

### 1. **\[BREAKING!\]** [`app`] [`rola`] [`layout`] Remove an entry from a Vault's Layout, by the account that holds it

`rola layout path remove <PATH> --layout truth@VAULT` drops the entry the path names from the
Vault's own Layout, and the path with it. Only the account that holds the entry, or one that
administrates the Vault, may remove it: another account's refusal names the holder, and an entry
nobody holds is an administrator's to remove. The local form is unchanged, `rola layout path
remove <PATH>` still only unbinds the path, leaving the entry the work's.

The write is the Vault's own: a new daemon action, `remove-remote-path` (id 19), carries the path
and the Vault answers with what became of it, so an offline run is refused rather than answered
locally, and what the Vault agreed to is written into the fetched copy as well, a reading command
sees the entry gone without a fetch of its own. What the entry's version names stays in the store,
and the version and the objects under it stay in the index: what is removed is the file as a thing
the Vault holds.

The desktop's upstream view follows: a row of a fetched copy the acting account holds is removable
with the browser's own `Delete` gesture and its delete menu item, which ask before removing as
every delete does. What the run came to is read back from the copy the run wrote rather than taken
from its status, so a removal that did not happen says so. An entry another account holds is not
offered there, since only its holder is known to the place: removing what an administrator does not
hold is the command line's.

Breaking: a Vault's Layout could not lose an entry before, and now can. An entry that is removed
there is no longer named upstream, so the next `rola sync` reads the work's own record of it as a
file the Vault does not hold and submits it again under the path the work tracks it at, unless the
work lets go of it too.

### 2. **\[BREAKING!\]** [`app`] [`rola`] [`retrack`] Record a version, or another entry, for a file

`rola retrack` now takes what the path is to record as its second word: `rola retrack <PATH> [TARGET]`.
The `--until` flag is gone, and so is the `--change-upstream` flag a earlier draft of this version
had: both are what `TARGET` says. One version back is what naming no target does.

A version target moves the entry's pointer as `--until` did: `~N` N versions back, `N` the version
numbered N, or a version's hash, a hash off the way back is a jump, and a jump is what a retrack
makes unless it is asked not to. `--only-index` leaves the file alone. A step count is a word, so it
is written `~N` rather than `-1`, which the command line reads as a flag; a run that writes one
anyway is told the spelling that reads. A Vault's name is no longer a target: what a target names is
a version, a `Uuid`, or a path of the Vault's own Layout.

An entry target (a `Uuid`, or a path of the Vault's own Layout) records that entry for the path in
place of the one it recorded, dropping that entry. What it holds is read from the fetched copy of
the Vault's Layout, or from this Layout when the `Uuid` is one it holds at no path; the entry must
hold no path of its own and must record a version. What happens to the file follows how the two
versions stand: the same version, or one the path's version is ahead of, is a change of identity
only and keeps the file as it is; a version behind it has that version's content written over the
file, since what decides whether a file may be written is the file's own state and not the flag,
changes that were never recorded are what `--force` discards, and `--only-index` records the entry
and leaves the file alone. Two chains are a jump, which a run makes unless `--deny-jump` refuses it,
the flag the version target's jump is refused with as well. `--fork` still records a fresh `Uuid`
with what the path records now, and cannot be given with a target.

What the command answers with is one shape whatever the target was: the path, the version it records,
and whether what it records changed. A run that recorded an entry also answers with the two `Uuid`s
and whether the entry was forked. The desktop's retrack passes the file and the target as its two
words and asks for no jump either way: a version the work's chain does not hold is a jump the command
makes, so the plugin no longer tells it apart.

### 3. **\[BREAKING!\]** [`app`] [`rola`] [`sync`] Submit a new file under the path the work tracks it at

A `Uuid` only the Workspace holds no longer reaches the Vault under `@/new/`: the Vault's
Layout names it at the path the work tracks it at, so `art/hero.psd` is an entry of the Vault's
Layout called `art/hero.psd` rather than `@/new/art/hero.psd`. A first submission reads where the
work keeps the file instead of in a staging tree of the Vault's, and the two trees line up from the
first sync.

Because the name is now the work's own path, a path the Vault's Layout already names is told apart
the way it was: the parent, the stem, the `Uuid` in short form and the extension,
`art/hero@1a2b3c4.psd`, with the short form growing a character at a time and the whole `Uuid` last.
The Vault side accepts any path a remote Layout may name, so no prefix is required of a creation.

What an older version submitted under `@/new/` is unchanged: such an entry is still read, moved and
deprecated as before, and a path under the marker is still a name a new entry may be made under.
There is no migration, a layout that holds both spellings is what one built across the change
looks like, and `layout path move` is what takes an old entry out of the marker.

### 4. **\[BREAKING!\]** [`modules`] [`daemon`] [`move-remote-path`] Move a path in a Vault's Layout without holding the entry

Moving a path in the Vault's Layout, locating an entry out of `@/new/`, deprecating one under
`@/removed/`, renaming or regrouping one anywhere, no longer requires the account that asks to
hold the entry, or to administer the Vault. A move changes where an entry is rather than who holds
it, so any account the Vault lets write its Layout may make one. `rola layout path move` therefore
no longer renders a refusal for an entry another account holds, and `rola req-ownership` /
`giveup-ownership` are about content writes rather than about moves.

The desktop's upstream view follows: every row of a fetched copy is renamable, and a folder is no
longer offered only when all of its entries are the acting account's.

`rola layout path move` still refuses a path that names nothing, a path that is already taken, and
a path a remote Layout may not name. Two moves racing are answered by the Layout's log rather than
by a holder check, the last write is the path the entry has, which is what a move always left
behind; what a caller that read a path and then moves it finds taken is answered as taken rather
than overwritten.

### 5. [`app`] [`rola`] [`inv-idx`] List the variants and versions the inverse index holds

`rola inv-idx list` lists what the records name: a variant is a row with its key, the version it is
based on, the variant that version fixes, the variant it merges in, and the content, creator and
message it names; a version is a row with its key, the variant it fixes, its number and whether it is
the root. The rows are the variants and then the versions, each in key order. `--kind
variant|version|all` keeps one kind, `--limit` stops the listing, and a kind this build does not know
is refused with exit code 134. A root is drawn with `is_root` and no number, since it is numbered
before the first.

`--json` writes the rows under `objects` with the same field names, and `--format` draws each row
through a template; the default is the kind and the key, a row a line. The listing is read-only: the
records are used when they describe the index, and the objects are read instead when they do not, so
a stale index is listed rather than rebuilt.

`rola inv-idx rebuild` now reads only the objects written since the last build instead of every
object, and its `--json` report says what it had to do: `mode` is `fresh`, `incremental` or `full`,
with `objects_read` and `versions_numbered` beside the counts it already had.

### 6. [`app`] [`rola`] [`layout`] Change or renew the Uuid a Layout path names

`rola layout path change-uuid <PATH> <UUID>` makes a path name another entry: the entry the path named
is dropped and the one given takes its place, holding the data the Layout already held for it. The
`Uuid` has to be one the Layout holds at no path, since an entry is at one path, another path naming
it is refused, and `rola layout path remove` is what lets it go first.

`rola layout path new-uuid <PATH>` drops the entry the path names and makes it again under a new
`Uuid`, holding what it held, so an entry can start over without the history it was recorded with
being touched.

Neither reaches beyond the Layout: the version index, the stores and the working tree know nothing of
a Layout's paths. A fetched copy of a Vault's Layout is refused as every Layout-writing command
refuses it, by the read-only name `truth@VAULT`. The Layout's own batch writes grow what the two
need: an entry can be removed in the same batch that binds its path to another, which is one write
and no moment where the path names nothing.

### 7. [`app`] [`rola-daemon-gui`] [`windows`] Give the Windows daemon window the dark theme of Windows 10 and later

`rola-daemon-gui.exe` now draws in the dark application theme of Windows 10 1809
and later. The title bar, the dialog ground, the buttons, edits, lists and the tab
strip all follow the desktop's theme, and a change of that theme is followed
while the window is open.

Dark is used only when the machine answers for itself. A build older than 1809,
high contrast, a theme value that cannot be read, or a uxtheme that does not
export the dark-mode entry points leaves the window light, and no state is half
dark. The tab strip is painted by the window after the control draws itself,
because the control's dark theme leaves the unselected tabs light.

### 8. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Draw the desktop Version Graph as the work's variant graph

The Version Graph dock now draws what the variant graph module reads: a card per variant, a lane per
file and a column per version depth, with the merges, the drafts and the entries laid beside them.
The old drawing was one file's chain, on a vertical version axis, made from the retired version-graph
reading; it is replaced rather than kept beside the new one, and the dock keeps its name and its
place in a saved layout.

A card is two rows: a description row with the creator's name, the message and the version number,
and a row of pills under it naming the variant's hash and the entries that point at it. The
creator's name is written in its own colour, not in a pill of its own, so the row of pills beneath it
is the only pill row on the card. The work's own
entries wear the look's accent and a Vault's the primary, and the chain of the entry being read is
outlined in the accent. The local pill of a file the user has chosen is drawn in the look's ink
instead, so the files a reading stands on are told from the rest of the work; choosing several marks
each of their pills. The entry at a chosen file's path on another side is the record that file tracks,
and its pill is drawn in the accent in both its border and its words. The graph stands on
the files chosen in a directory listing, or on the whole work when none is chosen, and a Vault's
fetched copy stands on the same lanes as the work. A merge that was recorded is a plain edge; a merge
still waiting is the amber broken one, both leave and arrive at the points the chain's own edges
use, at a card's middle, rather than hanging off its top or bottom. An edge says what it is by its
colour, its arrow and the cards it joins, rather than by a label over it. A file the work says is changed but not tracked stands as a
draft card, in the accent when a track would continue its record, red when the change is based on an older
version, and a base version the index
does not hold is a broken node beside the card naming it.

Hovering a card's message, a pill or the creator's name fills a table in the graph's corner with what
that thing says (the whole hash, the Uuid, the path, the owner, the reason a state is worn) and the
switch beside it turns the table on and off. A value wider than its column ends in an ellipsis, and a
right-click on the thing copies the whole of it; a right-click on a variant's hash also offers
retracking the chosen file to that version and joining the variant into it, and a right-click on
another side's entry offers switching the chosen file's record to that entry's Uuid, either moving the
record alone or letting the content follow it, an edited file is asked about before its content is
written over. A right-click on the work's own entry offers forking its path into a new Uuid, holding
what it held, which is how a file stops following an entry it was following. Pressing a local entry's
pill asks the browser to show the file it names. A card and the hash pill under it name one variant
and offer one menu: copying its hashes, joining it, ending a merge, and retracking the chosen file to
its version. A right-click on the card still offers the retrack,
join, cancel-merge and delete-changes runs the old graph offered. A press on a card brings it to the
middle of the dock, enlarged to 130% when the drawing reads smaller than that and left at its scale
otherwise, and a press on the empty ground takes the choice away, the
listing's rows and the graph's chains both, while a drag across the ground moves the drawing. The
choice's emphasis goes with it, but the cards stay where they are until another file is chosen, so a
click that chose nothing does not replace the drawing with every variant of the work. The
scale and place ease into view rather than snapping, and the wheel likewise. The drawing stands on a
grid of twenty-unit squares. Hovering an edge draws it heavier in the accent and lifts its two ends
over the rest of the cards, and the version columns carry no ruler or labels of their own: the column
position and the card's number are the reading. The drag, the wheel and `F` still move and frame the
drawing, and only the cards and edges the viewport holds are drawn.

### 9. [`modules`] [`inverse-index`] Read the inverse index the other way, and rebuild it incrementally

The inverse index can now be read as a snapshot: every variant by its key with the version it is
based on, the variant it merges in, and the content, creator and message it names; every version by
its key with the variant it fixes and its number. What a walk asks of the records, which versions
fix a variant, which variants are based on a version, which merge one in, which were made of one
content, is answered from that inversion rather than by reading the records again per step.

A snapshot also cuts a window of the variant graph: `Snapshot::graph` takes a focus, how far back
towards the root and forward away from it to walk, whether to follow merges and branches, and a cap
on nodes. What it answers with is the variants, the versions, the base and join edges, the keys the
index does not hold, and a `frontier` on each node saying which way the window stopped short;
`Snapshot::expand` walks one step further from a node. A branch is walked whole rather than stopped at
its first variant: the same variant may be advanced under another Uuid, and the versions that other
record went on to make are what a reader is looking for when it finds a fork. A branch is not taken
from the root: every file's first variant is based on it, so its dependents are the other chains
rather than this one's history, and a chain a merge brought in does not seed branches either. Nothing
is read from the objects and nothing is fetched: a version or variant the index does not hold is named
as missing.

A rebuild is now incremental. The keys the records cover are compared with the keys the index lists,
and only the difference is read; the file is built afresh only when it cannot be added to, it is
absent, does not read, or covers an object the index no longer holds. Versions are numbered by
filling outwards from the versions already numbered, so a version whose base was not written yet is
numbered by a later rebuild when the link arrives. The report now says which of the three happened
(`fresh`, `incremental`, `full`), how many objects were read, and how many versions were numbered.
The file format is unchanged, so existing records stay valid and the first rebuild after this takes
them in incrementally; `Edge::Join` gains the `join_dependents` question the other roles already had.

### 10. [`modules`] [`status`] [`variant-graph`] Read the variant graph of a workspace as one JSON document

`rorolala-status` gains `locate_variant_graph_json`, which reads the variant graph around the entries
a request names and answers with one JSON document: the graph, the entries that name it, the merges
in progress and the texts. The window is the one the inverse index walks, a focus of entries, each a
side and a path, how far back, forward and into merges to look, and a cap, and every node carries a
`frontier` saying which way the window stopped short, so a drawing that reaches the edge asks again
from the node it reached. A side is `local` for the Layout being worked in, or the name of a Vault
whose copy was fetched. The records are rebuilt first when they do not describe the index, which is
derived data and never the network; a key the index does not hold is named as missing rather than
fetched.

The words of each creator and message the window's variants name are read from the index once each
and carried beside the graph, so a drawing does not ask for them node by node. Every entry whose
version the window holds is named with the side it is on, the version it records, the account that
holds it, and whether it was moved under the deprecated marker; a merge in progress names the variant
waiting, the entry it joins into and where its variant file lies.

The C ABI exposes the same reading as `locate_rola_variant_graph_json`, whose answer is an owned
string and whose failures are a `RolaVariantGraphError`, nothing to draw, which is no Workspace, no
Layout or no index, told apart from a request or an index that could not be read. `modules/rola-sharp`
gains `RolaVariantGraph`, which asks for a window with a `VariantGraphRequest` and reads the answer
into records, and `modules/workspace` gains `Workspace::readonly_layout_copies`, which names the
Layout copies a Workspace keeps, by the Vault and the Layout each was fetched under.

## Refactor

### 1. **\[BREAKING!\]** [`app`] [`rola`] [`layout`] Remove the `.rolayout` file format

The `.rolayout` file is gone: `rola layout shot` no longer writes a Layout down as one, `rola
layout import` no longer makes a Layout from one, and the library has no reader or writer for the
format. A Layout is kept as the log it is and reached through the Layout itself, so a copy of one
moment of it written to a file is a second thing to keep in step rather than a way to hand work
over.

A caller that wrote a `.rolayout` file to move work between machines has no command to replace it:
the work travels as the Vault's own Layout, fetched from the Vault, and a Workspace made from
nothing is bound to a Vault and checks the work in from it. A file already written is not read by
this version and nothing converts it.

### 2. **\[BREAKING!\]** [`modules`] [`workspace`] Keep one Layout per Workspace

A Workspace works in one Layout. There is no set of named Layouts and no pointer naming the one
checked out: the Layout is `./.rola/layout/`, made with the Workspace, and it is the only one a run
can address. A Vault keeps its one Layout as before.

The commands that managed the set are gone: `rola layout new`, `rola layout cp`, `rola layout rm`
and `rola layout force-switch` are commands the program does not have. `rola bind` is what puts a
fresh Workspace to work, it binds the Vault and points the Layout at it when the Layout tracks
nothing, and `rola layout set-track <VAULT>` and `rola layout un-track` change what it tracks
without taking a Layout's name. `rola layout ls` lists the one Layout under the label `main`, and
`--layout` accepts `main` or a fetched Vault's `truth@VAULT`.

A Workspace made by an older version keeps working: the next time it is opened, the Layout it
worked in is adopted as its one Layout, and any other Layouts it held are left where they were and
are no longer reachable. Emptying a Vault's Layout is no longer offered by a command.

The desktop's upstream view is unchanged in what it lists and how it moves a path: its `layouts`
domain now holds the one Layout, addressed as `rola@layouts/main`.

# 0.1.3 (2026-11-01)

## Bugfix

### 1. [`app`] [`rola`] [`pager`] Never page a run that is not talking to a terminal

A run whose output or input is not a terminal now reaches no pager at all, whether or not it asked
for `--no-pager`. The pager already refused to start when its own stream was not a terminal, but
`ResNoPager` now says the same thing up front: a run read through a pipe is another program reading,
and a run whose input nobody types at would wait for a key forever. That is the difference between
an agent driving the command line and one that hangs on it.

`--no-pager` still works, and a run in a terminal pages as it did before.

### 2. [`app`] [`ROLALauncher`] Wait for the Desktop window before leaving the launcher

Opening a Workspace no longer closes the launcher as soon as the Desktop program is started. The
chosen card turns a mark and the rest of the window, the other cards, the buttons and the language
menu, is put out of reach while `rola desktop --ready` is waited for, and the launcher closes only
once the window has said it is up, so the two programs read as one. A launcher that is waiting also
refuses to leave: Escape and its own close button do nothing until the wait is over, which keeps the
start it made from being a process nobody is waiting on.

A start that ends before its window opens is reported with its exit code, and one that has said
nothing after thirty seconds is reported as a timeout. The launcher then comes back rather than
ending a start that may still arrive, and the Desktop process is left going, because a process
ended for being slow is a window lost. A start that found the Desktop program already running,
exit code 174, is answered as a window, since the run that was already up was asked to come
forward.

### 3. [`app`] [`RorolalaDesktop`] [`plugins`] Fix renaming a row in the desktop's upstream view

Renaming a folder in the upstream view (a place the Rorolala plugin answers for, `rola@…`) now renames the work's
paths. A folder that the copy of the Vault's Layout names is the work's, however long ago this side suggested it, so the
rename moves its entries to the new prefix instead of rewriting a suggestion and leaving the work where it was; the
empty folder at the new name, which was the suggestion left behind, no longer appears. A suggestion the copy has used
up is let go of rather than kept hidden, so a path the work has finished with does not bring one back, and a suggestion
the copy does not name is still moved by rewriting it.

The row a new folder makes is named the moment it is drawn. The hand-off is claimed by the listing built after the
place is read again, from the entries that have the folder in them, rather than by the view that asked for it, whose
rows are from before the making, and only by a view the user can see, so two docks over one place put the box in the
one on screen and not in a dock behind a tab.

A name being typed is no longer taken out from under the user: a read that lands while the box is up is drawn after
the edit rather than through it, the dock's own keys answer nothing while a row is an edit, and the name is put back
where the box was when the edit ends, a rename the place refuses therefore leaves the row showing the name it has
rather than a field nothing answers in. Removing a row from a place asks first unless Shift says not to, and the menu
on a row a place lets this side edit offers naming and removing it, so neither depends on a key.

The same hand-off is what names a new directory in the file system's own listing again: it too was being claimed by
the view that asked for it, which is not the view its row is drawn in.

### 4. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] Squeeze the directory dock's columns instead of cutting them off

The directory dock's table kept every column at the width it was given, so a dock narrowed
below the table's own width put the right-hand columns past its edge, scrollable to, at best,
and the list's horizontal scrollbar made sure the rows kept that width however narrow the dock
was. The table is now drawn at the width it has: its columns keep the widths they are wanted at
while there is room, and squeeze in proportion when there is not, so the whole table stays inside
the dock. The widths are worked out once for the table and written into the band of headings and
into every row alike, rather than each sheet working out a share of its own, so a column stands at
one place all the way down the table and the band's headings sit over the cells they name. The
horizontal scrollbar is taken off the list for the same reason: a list that can scroll sideways
measures its rows at the width they ask for, which is the width the squeeze would never reach.

### 5. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Fill the Overview's table to the dock rather than to its last row

The Overview's table stopped at its last row: the band and the rows were as tall as the rows came
to, and the rest of the dock under them was empty ground. The table now takes the whole of the
column the reading is drawn in, the band at its top and the rows filling the rest, and the rows
scroll inside the table when there are more of them than there is room for, rather than the whole
reading scrolling as one. What stands in the table's place when there is nothing to show is
unchanged.

### 6. [`utils`] [`cli-theme`] [`rendering`] Draw a three-dash Markdown rule as a rule

A message that marked a section break with `---` now draws a rule there, as Markdown says it should,
instead of printing the three dashes as ordinary text. The test that recognized one required more
than three dashes, so only `----` and longer were drawn; three is also the length a table's
separator row uses, but the table is taken before the rule is looked for, so the two do not meet.

A fenced code block still keeps its `---` as the text it is, and one or two dashes are still
ordinary text.

## Optimized

### 1. [`app`] [`ROLALauncher`] Open the launcher before reading what it shows

The launcher window is shown at once, holding a turning mark, and what it shows, the Workspaces
list, the account, how the machine draws a frame, and the welcome's lines, is read after it is up.
How a frame is drawn and what accounts exist are both answered by running the command line program,
and the window used to wait for those answers before it appeared. Now it appears first and is built
when the answers arrive, so that a launcher is something to look at while it finds out what to show.

A Workspaces file that cannot be read is an empty list rather than a window that refuses to open,
and a look or language change that arrives before the window has been read is held until the read
has drawn it.

### 2. [`app`] [`RorolalaDesktop`] Show the Desktop window before its plugins are assembled

The main window is now made and shown once the kernel's docks and menus, the look and the saved
layout are ready, and before the plugins are initialized. What is left of the host, the plugins and
the menus they register, the single-instance listener, the dock-state subscription and the layout
restore, and the report, runs afterwards, one step per turn of the window's thread, under a black
covering with a turning mark. The covering fades once the last step has returned, so the first frame
is the window rather than a half-made shell, and a slow plugin no longer holds the window back.

A step that throws is written to the log and the remaining steps go on: a plugin that will not start
is no longer a program that will not open. The menu bar is rebuilt once the plugins have registered,
a window opened again after the assembly has finished is made without a covering, and a
`rola desktop --ready` run is answered with one line on standard output as the window opens.

## Feature

### 1. **\[BREAKING!\]** [`app`] [`RorolalaDesktop`] [`docking`] Show which dock the user is working in

The dock the user is working in is marked: a one-pixel ring in the look's primary is drawn around the whole of its area
, the strip its tab stands in and the view under it, over the docks rather than laid around one, so the ring takes no
room from what it outlines and cannot be pressed by mistake. The ring follows the work: a press anywhere in a dock, the
keyboard going into one, a tab being chosen and a dock being opened all name that dock as the one being worked in, a
window coming forward names the dock it holds, and a dock moved into another group or window keeps the mark. When the
dock that had it closes, the mark passes to the one beside it, the other tab of its group, or a dock still open in the
window, and is let go when the last one that had it is gone, so it is never left around a dock that is not there. The
mark is drawn from the primary the look publishes rather than from a colour read when it was made, so a primary the
user changes reaches it with nothing further.

A plugin reads the same answer: `IDockFocus` says which dock is being worked in, the dock's stable name and which
instance of it, and is raised as it moves. That is what a dock's own keys are meant to be read against as they are
folded into the focus rather than bound over the whole program; the folding itself is not in this change.

**Breaking:** the plugin contract is 0.14.0. It gained the focus a plugin reads, the reveal a view asks the browser
with, the name a dock may answer for itself, and the picture a dock may be drawn by, each described in its own entry.

### 2. [`app`] [`rola`] [`desktop`] Ask the Desktop window to say when it is up

`rola desktop --ready` asks the window to write one line on standard output, `rola-desktop-ready`,
once it is on the screen. It changes nothing else about the run: the command still waits for the
window to close and still ends with the window's own exit code, and a run that did not write the
flag keeps the silence it had. It is meant for a caller that starts the command and has no window of
its own to watch, the launcher, which otherwise has to guess when there is something to look at.

`--ready` asks for a window, and `--query-window-style` asks how one is drawn and opens none, so the
two are refused together: the refusal is `ErrorDesktopQueryWithReady`, printed with its help line,
and exit code 176. The flag is offered by completion.

### 3. [`app`] [`rola`] [`docs`] Read and search the built-in book from the command line

`rola docs <TEXT>` reads one article of the built-in book by its exact name and prints it as it
was written, through the program's own Markdown rendering. `rola docs --search <TEXT>...` searches
the book instead: the words are read as one query and an article is found when every term of it is
in it, the answer is drawn across the terminal the way `gh issue list` draws a listing, no
borders, the header bold and underlined, the article's name as wide as the names that matched, its
description in a fixed column, and the matched fragment filling the rest of the width, the matched
words red and bold, and a Latin query is also found as a prefix or a substring of a word, which
the book's build indexes every word for.

`--limit <N>` caps how many search answers are shown, twelve by default and only meaningful with
`--search`. A search that finds one article, or one a run limited to a single answer, is answered
with that article rather than the table. `--raw` prints the article as it was written, Markdown
markers and all, rather than drawing it, and a `--raw` run is never handed to a pager. A `--json`
run is answered with the same result as fields: an article is its `title`,
`description`, `keywords` and `content`, and a search is its `entries`, each with the article's
name, title, description, score and fragments, and each fragment with its field, its text and the
ranges that matched. The global `--no-pager` is honoured like it is everywhere else, and a run
that names neither an article nor a query is shown the command's help.

The book is found beside the program an export or an install puts it beside, or, for a run made
from a checkout, above the directory it was run in, under `build/`; a checkout that has not run
`cargo dev book` is told the book is missing rather than answered with nothing.

### 4. [`app`] [`RorolalaDesktop`] [`docking`] Mark a dock by its picture in the Window menu and in its tab

A dock now says what it is with a picture as well as a name. The item that opens it in the Window menu draws that
picture in its own header, after the check that says whether the dock is open, the base theme keeps an item's check and
its picture in one slot, so the picture would otherwise be drawn over the tick, and its word is set the way a menu's
other words are, so that its row is the height of every other item of the menu. The left of the dock's tab wears the
picture too, in the ink of whatever it is drawn in: a strip of tabs says what is in a window by shape before it is read,
and a menu that lists a program's docks beside a file manager's is one a picture makes a row of rather than a list of.

The docks that ship are drawn by the pinned Material Design set: the Plugin Manager is a puzzle piece, the Log is a
list of lines, the Preferences are a cog, Directories is a folder, the Folder Tree is a tree of nodes, the Overview is
a checklist whose last line is ticked, the Workspace is the work as a case, the Version Graph is the way a change comes
round again through one file, the Accounts are the person the work acts as, and the Terminal is a window with a prompt
in it. A dock that names no picture has none, and its tab is the tab it was before.

A tab's picture is drawn at a whole share of the size the set draws it at, so its strokes land on pixels rather than
between them, and the room the tab keeps for it is on the picture's left: the word follows the picture as closely as it
followed the tab's own edge before there was one. The word in a tab is set at the weight the program's words are set at,
and the tab being read is not set heavier than the rest: the strip is a row of labels rather than of actions, and the one
being read is told apart by the ground it is drawn on.

A plugin says what its docks look like through the contract's newest field, a key into the icon library, the same
library a menu item's mark and an entry's badge are read from, so a registration carries a key and the picture travels
with the plugin that draws it.

### 5. [`app`] [`RorolalaDesktop`] [`docking`] Name a dock by the place it is looking at

A dock's tab now says where the dock is looking rather than only what kind of dock it is: a browser is one dock made
again for every place, so a strip of tabs all called "Directories" was a strip nobody could tell apart. What the tab
says is the real path of the place, the whole path of a directory, the whole address of a place a plugin answers for,
the drive where there is no name, and "This PC" where there is no path at all. The name is said again as the browser
steps, goes back or forward, is sent to an address, or has its base moved.

A name too long for the strip keeps its end and leaves its front out, since the last of a path is the place and its
first is only the drive it begins on; the whole of it is on the tab's tip and at the top of the tab's menu, and the
tab's width is capped besides, so one dock cannot take the strip from the docks beside it. A window that is one dock is
that dock: it is named by the same title, renamed with it as it goes, cuts it off at the front for the same reason, and
carries the whole place on its tip. The Window menu still names the kind, since its item asks for another dock of that
kind rather than for the one a tab holds.

The computer is a place a browser can be in, so it is named too, and a rule that runs a program is no longer asked
about it: browsing the drives with the Git plugin loaded used to fail where a repository was looked for from no path at
all. A dock says what it is called through the contract's newest answer, which can say both the name a tab carries and
the whole place it may be only part of.

### 6. [`app`] [`RorolalaDesktop`] [`plugins`] [`file-system`] Copy a cell's words from a directory listing's menu

Right-clicking a cell of a directory listing now offers to copy the words shown in it, a name,
a size, a date, the owner a column says, as the first entry of the menu the row already had. The
words are written on the entry, so a menu opened over a row says which of its cells it would copy:
the size column copies the size, the owner column copies the owner. What is copied is the cell's
whole text rather than the part the column had room to show, and it works in both arrangements the
dock reads entries in, the rows and the tiles.

### 7. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Show a path from an Overview row in its folder, with the version graph following it

The path a change is read by in the Overview is now a way into the work: pressing the From or To cell of a row asks the
browser to show the place the path names. A path of the work's is joined to the Workspace root the reading was made over
and shown in the folder holding it, with the file chosen, the same choice a click makes, so the Version Graph follows
it and the row can be looked at, and confirmed, from where it is shown. A cell that names an address is shown as the
address it is, since only the plugin that answers one knows its shape, and a side of a row with no path on it is a cell
that does nothing, marked with the pointer only where there is somewhere to go.

The place is shown in the dock the user last worked in, the browser the Overview was read from, rather than a second
one made beside it; one that is not open is made, and showing it brings that dock's tab forward and puts the keyboard
in its listing. A file that is gone, or which a hide rule leaves out of the listing, is shown by its directory alone; a
path whose directory is not there either is said rather than stepped to, so the dock stays where the user left it. A
dock is told what to show with the same request it is opened with, so a browser opened onto a place and one shown a place
answer the same shape.

### 8. [`modules`] [`book-search`] Search the built-in book by word, with the matched fragments

A book written as Markdown with a YAML front matter block is now compiled into one searchable
file, and opened by a crate that answers a query with the articles it found, their descriptions,
and the fragments the query matched in them.

**The file.** `cargo dev book` (the `rorolala-dev-book-index` tool) walks `book/**/*.md` and writes
`build/docs/builtin.book`, one file holding the articles and the index over them. An article's name
is its path under `book/` with the extension taken off; its title, description and keywords come
from the front matter, and the title falls back to the first heading, then to the file name. Both
the article as written and its plain text are kept: the index is built from the plain text, and the
Markdown is there for a reader that draws it. The header carries a BLAKE3 digest of the body, so a
file cut short or written by a format this build does not know is refused rather than searched.
`cargo dev export` and `cargo dev desktop` write the file themselves, and the Windows installer,
the Linux packages, the AppImage and the Unix archive all carry it beside the programs, under
`docs/`, where `BookSearch::locate` walks up to it from `bin/rola` and from `bin/desktop/…` alike.

**The search.** Text separated by spaces is cut into lowercased words; Chinese-style text, Han,
kana, hangul, is cut into every character and every adjacent pair, so a one-character query finds
its line and a longer query asks for its phrase. An article is a hit only when every term of the
query is in it, in any of its five fields (name, title, description, keywords, content) and the
hits are ranked by BM25 with a title match worth more than a body match and the name breaking a
tie, so the same query over the same book answers in the same order. Each hit is one short
fragment, cut from the field that matched with `...` where it was cut, and the fragment's ranges
count characters rather than bytes and are already merged where two ranges overlapped.

**The fragments.** The build also cuts every word of a spaced script into runs of two and three
characters and writes them as a second index, so a query that is a prefix or a substring of a word,
`inst` or `stall` of `install`, finds the word without the book being read. A term found whole
ranks above one found through its fragments, and a fragment candidate is checked against the
article's own words before the article is answered, so fragments that landed in two different words
are not answered as one. The file is larger for it; the search is not.

**The surfaces.** The crate is re-exported as `librorolala::book_search`. The C ABI exposes a book
handle opened from a path or located beside the programs, a search that answers with a hit list,
and accessors for a hit's name, title, description, score and fragments; every handle and every
string is released with its `free_*`, and `tests/ffi-c/ffi-book` checks the nesting as a C11
program. `modules/rola-sharp` gains `RolaBookSearch`, whose `BookSnippet` splits its text into
ordinary and matched runs, so a renderer draws the highlight without converting the ranges itself.

### 9. [`utils`] [`cli-theme`] [`rendering`] Lay out a Markdown document for reading

`TextRendering::render_document` draws a document rather than a line: a heading is set in two
columns from the left with its `#` marker kept and its title bold on the terminal's reverse-video
ground, one blank line above and below; prose is set in four columns and wrapped at eighty-six
columns of content; a quotation keeps its `|` gutter on every wrapped line; and a table is set in
with the prose, a long cell folding inside its column so the table fits the document instead of
running off the side. The `rola docs` reader draws articles through it, so a book page now reads as
a page rather than as the Markdown the index carries.

`trd!` and `TextRendering::render` still draw a line where it was written, so messages, status
lines and help keep the shape their callers chose.

### 10. [`utils`] [`cli-theme`] [`table`] A full-width listing table for the command line

The command-line theme now carries a `Table` for the listings a `gh issue list`-style table is made
of: columns with titles and rows of cells, drawn across the terminal instead of only as wide as
their content.

A column is as wide as it needs (`Column::auto`), exactly as wide as it is told
(`Column::fixed`), or filled with what the others leave (`Column::fill`), which is what makes a
table reach the terminal's right edge; every line is padded to that whole width. There are no
borders: the header is bold and underlined, the columns are separated by two spaces, and a cell's
`\n` starts another line of the same row. Widths are terminal columns rather than characters, so a
table of Chinese text lines up with one of Latin text, and words wrap inside their column rather
than pushing it out of shape.

A cell holds runs (`Cell`, `Run`, `RunStyle`), so a caller can draw part of one another way, the
red and bold a search puts on a matched word, without handing the table a colour language to
parse. `rola docs --search` is the first user: it draws the article's name in an auto column and
the matched fragments in a filled one.

### 11. [`utils`] [`cli-theme`] [`theme`] Choose the Pretty theme where the terminal can be detected

`ThemeChoice::auto_select` now answers `Pretty` on a terminal that names itself one of the modern
terminals, instead of leaving `Pretty` to an explicit `--theme-choice pretty`. The list is Windows
Terminal, Zed, WezTerm, kitty, Ghostty, iTerm, VS Code's terminal, Hyper, Alacritty and foot; it is
read from `WT_SESSION`, `ZED_TERM`, `TERM_PROGRAM` and the other variables those terminals set about
themselves. A listed terminal is trusted for its 24-bit color as well as its glyphs, so Windows
Terminal and Zed do not have to set `COLORTERM` to be recognized; `COLORTERM` and `TERM` are still
read for a terminal that names itself some other way.

A terminal carries no answer about its font, so this is an approximation: a terminal outside the
list gets `Simple`, and one whose font turns out to have no glyphs is still answerable with
`--theme-choice simple`. A run whose output is not a terminal still gets `Text`, and an explicit
`--theme-choice` still overrides the detection.

## Refactor

### 1. [`utils`] [`cli-theme`] [`style`] Keep the Pretty line prefixes the same as the Simple ones

`err_line`, `warn_line` and `help_line` now write the same `Error:`, `Warning:` and `Help:`
prefixes on every terminal theme. The theme that carries true color used to replace the prefix with
a `NerdFont` mark and a coloured badge, so the same line read as two different shapes depending on
the terminal; it now keeps the name where a reader already knows it and gives it a brighter version
of the level's colour instead, red, amber and green, still apart in hue. The message is still
italic, and an alert, a rule and a table still use the glyphs where the theme has them.

# 0.1.2 (2026-10-29)

## Optimized

### 1. **\[BREAKING!\]** [`modules`] Carry a sync's bytes as bytes all the way down, from the packet to the pack

A sync of a Workspace and a Vault is now about twice as fast for a store of large chunks and some half
again as fast for one of small entries, and it asks the allocator for a third to a quarter of the
memory it used to: measured over the same runs of `dev/sync-harness`, a store of 500 three hundred
kilobyte chunks went from 934 ms to 456 ms, one of 2000 four kilobyte entries from 266 ms to 163 ms,
and the bytes asked for from 1 878 MiB to 447 MiB and from 154 MiB to 67 MiB.

The path a sync takes was bytes the whole way down and serde the whole way, and each layer copied what
the one above it had already copied. It now carries what it is handed:

- **A frame of its own.** A packet of entries, a blob and its pieces, an answer that is one byte per
  key, a confirmation: each of those is bytes already, and each was put through `bincode2` first, a
  copy into an encoded buffer and a visit per element. They cross as a length and their bytes now, in
  as many parts as the caller holds, and a packet's entry headers are built where the entry is added
  rather than when the packet is sent. What a value that really is a number or a struct pays is
  unchanged; it goes through the same frame.
- **Where they arrive.** The side taking a packet reads its entries out of the frame it received
  instead of copying each one out, and writes them into the store from where they lie.
- **Records sealed where they lie.** A record is read into the buffer it will be opened in, opened in
  place, and handed out of that buffer; the plaintext of a record going out is appended straight into
  the buffer that will carry its ciphertext and sealed there. What was four copies and a queue on the
  way in, and several on the way out, is one each way.
- **A packet is one record.** A record carries a megabyte, and a packet is sized to fit one: one
  sealing and one write per packet rather than sixty-four. A content of three hundred kilobyte chunks,
  which is what a cut PSD looks like, can now read its chunks with the window always full, the reads
  for the next packet are begun while this one is being sent.
- **Confirmed by what was written.** The side taking a direction answers "I have it" from the objects
  its batch accepted and the manifests it wrote, rather than reading the store back once the direction
  was over: on a store of many small chunks that pass cost about as much as carrying them did. What it
  gives up is one thing, and it is deliberate: a chunk written and corrupted before it is read again is
  no longer noticed at the end of a sync. Every entry is still checked against the key it arrived under
  as it arrives, and every read checks what it produces, so what is lost is the window between writing
  a pack and reading it in the same run.
- **Read and written through one opened batch.** The chunks of a content are read through one batch:
  each pack is opened once and its entries read where they lie, where a read used to open the pack,
  seek into it and read. The side sending a content no longer hashes each chunk before it goes, the
  side taking it checks every entry against the key it arrived under, which is the boundary that
  decides what a store holds. A payload stored as it is _is_ the content, so it is moved down inside
  the buffer it arrived in rather than copied into another.

**Breaking:** two things on the wire changed, so a Vault and a client have to be of the same version.
What a value's bytes are is stated by the new build for everything that was already bytes, a packet, a
blob and its pieces, a presence answer, a confirmation, and a record's plaintext is a megabyte where it
was sixteen kilobytes, which a reader of the old version refuses. A session also holds about three to
four megabytes now, between the buffer it reads into, the buffer it writes out of and the packet being
built, so the Vault's default of sixteen sessions at once is some tens of megabytes.

`dev/sync-harness` gained `--allocs`, which is where the memory figures above come from: the numbers
this work was measured and ordered by, rather than counted from the code.

## Feature

### 1. [`app`] [`rola`] Read, select and fetch the work by the managed ignore rules

The commands that read the tree now read it by the Workspace's managed ignore rules. `rola status`
and `rola layout tree-diff` leave out a path the rules govern, and `--include-ignored` shows it as
well; what the Layout names is still read, so a recorded file a rule matches appears as the change
it is, and a move the reading found keeps its place whatever the rules say about where it went.

`rola track` and `rola align` refuse a path the rules govern, naming an ignored file or directory,
or a directory selection that is itself ignored, with the argument code the command already has
(201 and 221) and a line pointing at `--include-ignored`, which is the run saying it means it. A
directory named for recording still expands over what it holds silently: a file under it that the
rules govern is not selected, and there is nothing to refuse because it was never named.

`rola fetch` brings the Vault's ignore rules down with its Layout, by default, into
`.rola/info/ignore/vaults/<NAME>/`: replacing whatever was there, so a rule the Vault gave up is
gone here too. `--no-ignore-rules` fetches the Layout alone and leaves the rules here as they are.
The fetch is the one Vault's, named or default, and never another bound Vault's.

What a rule set that cannot be read costs is a word on standard error and no rules for that run,
rather than failing a reading that has an answer: the reading is the thing being answered, and the
rules are a refinement of it.

### 2. [`app`] [`rola`] Speak every action to a Vault behind SSH, one ssh per action

Every command that speaks to a Vault now reaches one written as an scp destination. The client
was already one place, `proc_action`, so what changed is what happens there: instead of a
connection to a host and a port, the address is dialled, and a session becomes a process.

```text
ssh -T -o BatchMode=yes [-p PORT] -l USER -- HOST "rola-daemon serve 'PATH'"
```

The path is quoted the way a POSIX shell reads one back as the single word it was, which is what
makes a Vault in a directory whose name carries a space or a quote reachable at all. The account
travels as an option's value rather than as part of `user@host`, so nothing parses it twice, and
`--` is what keeps a host that starts with a hyphen where a destination is expected.

`BatchMode=yes` is fixed rather than chosen. The caller's standard input is the protocol, so a
password or a passphrase can never be typed into it: a session needs a host key already known and
an agent or a key without a passphrase, and a window that has no terminal at all fails at once
instead of waiting for what can never be typed. What `ssh` says about a refused key or an unknown
host goes where the command's own errors go, the standard error, and is not intercepted.

What the program ends with is part of the answer, so it is watched from the moment it starts
rather than from the moment its output ends: a session that failed before its first write, an
`ssh` that could not reach the host, is exactly the case with no output to read. A leg that cannot
be reached is a failure of its own, reported as `ActionError::Transport` and exit 152, and it says
which leg: no `ssh` on this machine, no `rola-daemon` on that one, a daemon too old to know
`serve`, a path that is not a Vault, a machine that refuses sessions, preferences that will not
read, or an output that carried something the protocol is not.

While a session lives, `SIGPIPE` is set aside: this program asks the kernel to make it fatal so
that `rola | head` dies the way a filter is expected to, and a program that ends before it reads is
the one thing that would then take the run down with no word about why.

One action is one session, and one `rola sync` is several actions, so a run may pay several
handshakes. That is not hidden: `ssh`'s own `ControlMaster` is what turns them into one
connection, and a machine that pays it per action is a machine that has not configured it.

### 3. [`app`] [`ROLALauncher`] Read the scp spelling of a Vault address in the window that creates a Workspace

The address a window is being edited with is read by the launcher before the command line is run,
which is a second reading of the same grammar: what the window accepts has to be what the command
accepts, or a window takes an address the command then refuses. It now knows the scp spelling too,
`user@host:path`, with the SSH port between the host and the path when one is named, and refuses
what the command refuses: no account, no host, no path, a port no session is made to, two colons
after the host, a path the far side would read as a flag.

What a failed check says is unchanged and is what a person needs for this one: the command line's
standard error, which carries what `ssh` said about the host key, the key, or the name that would
not resolve.

### 4. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Hide what a Workspace's ignore rules govern in the browser

The Rorolala plugin now contributes a hide rule to the File System plugin's catalogue, offered as
`rola_ignored` and labelled with the plugin's own key. A directory under the root being read is
looked up for the Workspace holding it, the Workspace's rule set is loaded once per Workspace
through the C ABI (off the thread the rows are drawn on, the way the provider contract asks) and
an entry the rules match is left out of a listing or a tree. Letting go of the answer when the files
may have changed is what makes an edited or freshly fetched rule file seen.

What the provider answers is the rules alone: an entry they match is hidden whether or not the
Layout records it, where the reading `rola status` makes keeps a recorded path out of the rules'
jurisdiction. That is the one place the browser and the reading can disagree, and it is the cost of
answering about a row without walking the tree the reading walks.

### 5. [`modules`] Manage ignore rules in one directory, and let a Vault hand rules down

Rorolala now reads its ignore rules from one managed directory, `.rola/info/ignore/` in a
Workspace, `./info/ignore/` in a Vault, instead of from the tree, and a fetch brings a Vault's
rules down with its Layout. A rule file is held under the name its author gave it and holds
gitignore's own syntax: negation, directory-only rules, anchoring and `**` all read as git reads
them. A file that the rules govern is not the work's gain, so a tree reading leaves it out of what
the tree gained; what a Layout already names is not the rules' to hide, and a move the reading
found is a fact about the work rather than something a rule suppresses.

The order the rules apply in is the managed directory's own answer: a deeper rule file is applied
before a shallower one, and beside its siblings a file whose name sorts sooner (numbers by value)
is applied before one that sorts later, so the last rule that matches a path is the one that
decides. A Vault's rules land under `vaults/<name>/`, deeper than the Workspace's own, which is
what lets a Workspace's `!*.mb` suppress a Vault's `*.mb`.

The rules themselves are matched by ripgrep's `ignore` crate rather than by a second
implementation of gitignore's language, and the set crosses the C ABI as an opaque handle
(`load_rola_ignore_rules`, `rola_ignore_rules_is_ignore`, `free_rola_ignore_rules`) so a program
outside Rust asks the same set the command line does.

`modules/tree-analyze` takes the rule set as part of a reading and filters only what the tree
gained, after the moves are paired. `modules/status` reads with it, so a status made of a Workspace
carries the same answer. `modules/daemon` gains the `fetch-ignore-rules` action (id 18), which
hands a Vault's whole rule directory over and replaces the Workspace's copy of it as one swap.
A rule the Vault deleted is gone from here after the next fetch, without either side keeping a
list of what used to be.

### 6. [`modules`] Reach a Vault over an SSH session, with no port bound at either end

A Vault can now be served to a caller that arrives through `ssh`, which binds no port and needs
nothing of the network beyond the `ssh` program the caller already has. The far side runs
`rola-daemon serve <PATH>`: one connection is carried by that process's own standard input and
output, and the process ends when the connection does. The table of namespaces is not read in that
mode (the path on the command line is the Vault, taken exactly as written) so a machine with
nothing registered can still serve one.

An address says which of the two ways to reach a Vault it means. The SSH spelling is scp's, with
the SSH port between the host and the path when one is named, and no port when one is not:

```text
user@host:path
user@host:port:path
```

Nothing that was written before is read differently: an account is what tells the two apart, and
no daemon address ever carried one. `VaultAddress` gained `link()`, which answers the daemon or
the session the address names, and lost `port()`, `authority()` and the `set_*` methods, which
described the daemon way of reaching one and not the other.

`modules/daemon` reads `rola/daemon/preference.toml` beside the namespace table, two switches,
both defaulting to the shape the feature was designed for, and written down with those defaults the
first time a session asks and finds no file there. A machine that has said nothing is a machine
whose switches a person still has to be able to find: the first session writes them, so turning one
off is editing a file rather than knowing where it would have been. A data directory that cannot be
written does not refuse the session, the defaults are what a machine that says nothing means, and
says on the standard error that the switches could not be kept.

```toml
[security]
ssh-connection = true
auto-register-by-ssh = false
```

`ssh-connection` refuses a session before any Vault is looked at. `auto-register-by-ssh` lets a
connection write the key it proved into the Vault's own keys when the name it uses has none there,
which turns the first connection of somebody the Vault has never seen into a member with no hand
placement of a key. It never writes over a key that is already there, never registers a name in
`[auth] admins`, an administrator may move any file in the Vault, and never registers anything
where the Vault's `key_discovery` does not name its own keys. Turning it on changes what revoking
a member means: a deleted key is written back by the next connection, so revoking is turning the
switch off or closing the SSH access.

The mode has two contracts that a caller depends on. The standard output carries the protocol and
nothing else, so a login script that prints breaks the session; everything the daemon has to say,
including the audit line of a registration, goes to the standard error, which `ssh` forwards to
the caller. And the exit code is the daemon's own, 4 for a path that is not a Vault, 5 for a failed
session, 6 for a machine that refuses sessions, 7 for preferences that will not read, because that
number is all the other end has to tell one failure from another.

Both switches are policy rather than a security boundary: nothing in a process can prove it was
started by `sshd`. What actually decides what a session may do is who may run `rola-daemon` as
that user, which is why pinning the far side's `authorized_keys` entry to `command=` is what the
switches are for.

# 0.1.1 (2026-10-28)

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
A shard is now read whole, its snapshot and the log after it under the log's shared lock, which is the
lock a compaction holds exclusively for the whole of what it does. The open also builds what it answers
from the shards it just read rather than refreshing a second time, which is one read fewer of every
shard.

A compaction passes over a shard whose log holds no bytes: there is nothing to fold, and what it would
spend is a flush of a snapshot that is what is already there.

A Vault also folds its Layout's log into the snapshots when the log has outgrown them, past a
mebibyte, or more than what the snapshots hold, which is what keeps the reading an open does from
growing with the history a Vault has served. A compaction that fails is left to the next action rather
than failing the one that was asked for.

### 3. [`modules`] [`tree-analyze`] [`cache`] Keep a shared analysis cache whole while another reading writes it

The cache a reading of the tree writes down is now written beside itself and moved over the file that is
there, rather than written over it. One work is read by several readers at once, the desktop's Overview and
each listing of it read it on threads of their own, and a reader opens either the cache that was there or
the one being written, and never a file that is part of each.

A reader that caught the old file between the truncation and the new bytes took it for a cache written by
another build and started from no agreements at all. It then wrote the tree as it stands as what the Layout
agrees with, and the changes it was about to report (a file edited without holding it, say) were gone from
every reading after that until the files changed again.

## Optimized

### 1. **\[BREAKING!\]** [`modules`] [`daemon`] Bring the chain under a version from a Vault in one exchange

A retrack may move the pointer to a version the Vault holds and this side never fetched. What it needs for
such a version to be readable here is the chain under it (the version, the variant that produced it, the
words they are named by, and so on down to a version this index already holds), and reading it one object
per exchange was a connection and a handshake per object.

It is one exchange now: the Workspace says the hash it wants the chain of and what it already holds, the
Vault walks down until it meets something the Workspace has, and what it found crosses back together, to be
written here as one pack. The walk stops at the root version rather than failing on the variant a root
version names and does not have.

The action is `bring-remote-index`, id 17. **Breaking:** a daemon and a program have to be built together: a
program that asks for the action against a daemon which does not serve it is refused.

### 2. **\[BREAKING!\]** [`modules`] [`daemon`] Open the Vault's Layout once per action and write each run as one batch

The actions that change the Vault's Layout no longer open it once per entry, and no longer write one
change at a time. An ownership run (`hold`, `giveup`, and the two `rola layout` doors) opens the
Layout once, decides every `Uuid` against one batch, and commits it: one reading of the shards and one
flush per shard, where it used to be sixteen shard reads and a flush for every entry. The `Uuid`s cross
as one value and the outcomes come back as one, so a run of a thousand entries is two values instead of
a thousand round trips.

A write that is made of many entries (`write-remote-entries`, which a sync's push uses) is one batch
as well, and a create is one record rather than a path and an entry that a crash could part.

The batch is committed before the outcomes are answered, and the side that asked is told whether it was
written before it is told what became of each entry, so an outcome is never one the Vault did not keep.
A commit that fails is reported with what it failed with.

**Breaking:** what an ownership run and a many-entry write exchange has changed shape. Both ends run the
same action and have to be built together: a Vault served by an older daemon, or a program built before
this, does not understand the exchange.

### 3. **\[BREAKING!\]** [`modules`] [`daemon`] [`fetch-layout`] Carry the Vault's Layout as the whole it holds, and pass over a fetch whose copy is already it

A fetch no longer carries the Vault's Layout directory as it stands. It carries the layout written out
whole (every shard a snapshot of everything it holds, with an empty log after it) because a directory
read while another action writes or compacts is the two halves of two states, and what a copy is for is
one whole.

A fetch that asks with the state its copy already holds is answered "unchanged" and carries nothing, so
a run whose layout has not changed since the copy was taken pays a round trip rather than a directory.
What a layout is written out as depends on what it holds and not on how it is kept, so a compaction does
not make a copy stale; and because the state is read back from the copy, a copy that was written to
after it was fetched, an ownership change followed into it, answers for what it now holds rather than
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
Vault brings is written as one pack here, one file and one index for a chain of any length.

What a run is told when the chain cannot be walked is unchanged: an object under the hash that is not a
version is refused as a level that is not a version, and a hash the chain could not go past is named.

### 6. [`app`] [`rola`] [`track`] Pack an index with the objects a recording made, and gather the packs when due

Every recording packed the index, and packing rewrites every object the index has ever held: the cost of
a run grew with the index for a saving that did not. A recording now writes the objects it makes as one
pack, one file and one index for the whole run rather than a file per object, and gathers what has
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
putting every content back together and hashing it, one reading of the store where there were two.

The one thing this makes stricter: a key a store holds but cannot produce, a manifest whose chunks are
gone, is now one both sides are asked for, and neither holds, so the run says so instead of leaving it
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

A run of index objects (what a sync takes, what a recording makes) is now written through a batch: the
objects are collected and laid down as one pack with one index, rather than a file, a rename and a
directory entry for each of them. What a batch takes is what a transfer takes: an object is decoded and
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
are drawn over it, four trapezoids along its edges, one for each way it may be split, and a centre
rectangle for joining its tabs, and the pointer is always on one of them. A drop on the centre joins
the group's tabs at the end of their order; a drop on an edge splits the group.

The group last clicked or focused is the active one, and a dock opened without a place of its own goes
there. The chosen tab of every group is remembered, and a dock moved into a window of its own returns to
the group it came from when that window goes.

The saved layout is version 2: the tree with a ratio for every split, the active group, and each group's
chosen tab. A version 1 file states placements and pixel widths, which cannot be turned into a tree
without inventing a window size; it is not converted, the starting arrangement is used, the log says
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
no Rust. It shows the same four pages as the Windows window, the daemon and its output, the namespace
registry, the user keys, and the Vaults' administrators, and every action behind them is still one
the command-line programs answer.

It reads the user's files where the Rust side does: `$ROLA_DATA_HOME` or `$XDG_DATA_HOME`, falling
back to `~/.local/share`, and then `rola/keys` and `rola/daemon/registry.toml`. `cargo dev export`
builds it on Linux into `build/bin/rola-daemon-gui`, after the programs it drives, so `rola-daemon`
is beside it. Building it needs GTK 4.10 or newer with its development files.

Stopping a daemon the window started asks it to terminate and falls back to killing it, because
`rola-daemon` has no stop command.

### 4. [`dev`] [`rola-build`] [`publish`] Package the Linux hand-off as deb, rpm, Arch and AppImage

`cargo dev publish` on Linux now lays the export out as the packages a distribution installs,
`rorolala-vcs`, `rorolala-desktop` and `rorolala-dev`, a `rorolala-full` meta package that names
all three, an all-in-one `rorolala` package that holds them, and a launcher-only AppImage. The
Desktop program and launcher are published self-contained, so the packages that hold them carry the
.NET runtime; the AppImage carries `rola`, `ROLALauncher` and `RororalaDesktop`, and no other
program.

A build for a musl target makes one package instead, `rorolala-musl`: the same all-in-one, with the
command-line programs built against musl and the Desktop side, .NET, GTK and the C ABI's shared
library, the host's own, since those are glibc wherever they are built.

The packages install the six programs, both desktop entries, the icons and completions, and a user
`rolad.service`. The unit is installed but never enabled or started by the package: whether the
daemon runs is its user's decision. Package metadata is read from the workspace's own `Cargo.toml`,
and nFPM and appimagetool are pinned beside the export.

### 5. [`modules`] [`define-format`] Add rola-define-format, the rola.lock format

`rola.lock` names the materials a project places and the Vaults their content can be fetched
from. It is generated, `rola.toml` is the file a person writes, so the grammar is deliberately
small: a `? FORMAT 1` line, one `? VAULT <name> <url>` line per source, and one
`<path> blake3:<hash>` line per material, with no patterns, conditions, inheritance or quoting.

Reading accepts a carriage return before a newline, because a Windows checkout may put one there,
and refuses everything else the grammar does not name rather than guessing at it. Writing has one
spelling of every lock, so two locks holding the same materials are the same bytes. A lock naming
a format version this build does not know is refused whole.

Paths, Vault names and content hashes are validated types rather than strings, and the crate is
the only place a lock is read or written: it holds no files, no network and no git.

### 6. [`modules`] [`git-support`] Add rola-git-support for project files and the work tree

A project is a directory holding `rola.toml`, the Vaults, the features, and one `[[mapping]]` per
material, with the generated `rola.lock` beside it. This crate reads both, finds the project and
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
