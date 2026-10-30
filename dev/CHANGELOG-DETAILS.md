# CHANGELOG

This document records all Changelog entries for Rorolala VCS starting from 0.1.0, managed by the `cargo changelog` command built into this project.

# 0.1.3 (Unreleased)

## Bugfix

### 1. [`app`] [`rola`] [`pager`] Never page a run that is not talking to a terminal

A run whose output or input is not a terminal now reaches no pager at all, whether or not it asked
for `--no-pager`. The pager already refused to start when its own stream was not a terminal, but
`ResNoPager` now says the same thing up front: a run read through a pipe is another program reading,
and a run whose input nobody types at would wait for a key forever. That is the difference between
an agent driving the command line and one that hangs on it.

`--no-pager` still works, and a run in a terminal pages as it did before.

### 2. [`utils`] [`cli-theme`] [`rendering`] Draw a three-dash Markdown rule as a rule

A message that marked a section break with `---` now draws a rule there, as Markdown says it should,
instead of printing the three dashes as ordinary text. The test that recognized one required more
than three dashes, so only `----` and longer were drawn; three is also the length a table's
separator row uses, but the table is taken before the rule is looked for, so the two do not meet.

A fenced code block still keeps its `---` as the text it is, and one or two dashes are still
ordinary text.

## Feature

### 1. [`app`] [`rola`] [`docs`] Read and search the built-in book from the command line

`rola docs <TEXT>` reads one article of the built-in book by its exact name and prints it as it
was written, through the program's own Markdown rendering. `rola docs --search <TEXT>` searches
the book instead: an article is found when every term of the query is in it, the answer is drawn
across the terminal the way `gh issue list` draws a listing — no borders, the header bold and
underlined, the article's name as wide as the names that matched and the fragments filling the rest
of the width, the matched words red and bold — and a Latin query is also found as a prefix or a
substring of a word, which the book's build indexes every word for.

`--limit <N>` caps how many search answers are shown, twelve by default and only meaningful with
`--search`. A `--json` run is answered with the same result as fields: an article is its `title`,
`description`, `keywords` and `content`, and a search is its `entries`, each with the article's
name, title, description, score and fragments, and each fragment with its field, its text and the
ranges that matched. The global `--no-pager` is honoured like it is everywhere else, and a run
that names neither an article nor a query is shown the command's help.

The book is found beside the program an export or an install puts it beside, or — for a run made
from a checkout — above the directory it was run in, under `build/`; a checkout that has not run
`cargo dev book` is told the book is missing rather than answered with nothing.

### 2. [`modules`] [`book-search`] Search the built-in book by word, with the matched fragments

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

**The search.** Text separated by spaces is cut into lowercased words; Chinese-style text — Han,
kana, hangul — is cut into every character and every adjacent pair, so a one-character query finds
its line and a longer query asks for its phrase. An article is a hit only when every term of the
query is in it, in any of its five fields — name, title, description, keywords, content — and the
hits are ranked by BM25 with a title match worth more than a body match and the name breaking a
tie, so the same query over the same book answers in the same order. Each hit is at most three
fragments, cut from the field that matched with `…` where they were cut, and each fragment's ranges
count characters rather than bytes and are already merged where two ranges overlapped.

**The fragments.** The build also cuts every word of a spaced script into runs of two and three
characters and writes them as a second index, so a query that is a prefix or a substring of a word —
`inst` or `stall` of `install` — finds the word without the book being read. A term found whole
ranks above one found through its fragments, and a fragment candidate is checked against the
article's own words before the article is answered, so fragments that landed in two different words
are not answered as one. The file is larger for it; the search is not.

**The surfaces.** The crate is re-exported as `librorolala::book_search`. The C ABI exposes a book
handle opened from a path or located beside the programs, a search that answers with a hit list,
and accessors for a hit's name, title, description, score and fragments; every handle and every
string is released with its `free_*`, and `tests/ffi-c/ffi-book` checks the nesting as a C11
program. `modules/rola-sharp` gains `RolaBookSearch`, whose `BookSnippet` splits its text into
ordinary and matched runs, so a renderer draws the highlight without converting the ranges itself.

### 3. [`utils`] [`cli-theme`] [`table`] A full-width listing table for the command line

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

A cell holds runs (`Cell`, `Run`, `RunStyle`), so a caller can draw part of one another way — the
red and bold a search puts on a matched word — without handing the table a colour language to
parse. `rola docs --search` is the first user: it draws the article's name in an auto column and
the matched fragments in a filled one.

### 4. [`utils`] [`cli-theme`] [`theme`] Choose the Pretty theme where the terminal can be detected

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
of the level's colour instead — red, amber and green, still apart in hue. The message is still
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
  key, a confirmation: each of those is bytes already, and each was put through `bincode2` first — a
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
  which is what a cut PSD looks like, can now read its chunks with the window always full — the reads
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
  seek into it and read. The side sending a content no longer hashes each chunk before it goes — the
  side taking it checks every entry against the key it arrived under, which is the boundary that
  decides what a store holds. A payload stored as it is *is* the content, so it is moved down inside
  the buffer it arrived in rather than copied into another.

**Breaking:** two things on the wire changed, so a Vault and a client have to be of the same version.
What a value's bytes are is stated by the new build for everything that was already bytes — a packet, a
blob and its pieces, a presence answer, a confirmation — and a record's plaintext is a megabyte where it
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

`rola track` and `rola align` refuse a path the rules govern — naming an ignored file or directory,
or a directory selection that is itself ignored — with the argument code the command already has
(201 and 221) and a line pointing at `--include-ignored`, which is the run saying it means it. A
directory named for recording still expands over what it holds silently: a file under it that the
rules govern is not selected, and there is nothing to refuse because it was never named.

`rola fetch` brings the Vault's ignore rules down with its Layout, by default, into
`.rola/info/ignore/vaults/<NAME>/` — replacing whatever was there, so a rule the Vault gave up is
gone here too. `--no-ignore-rules` fetches the Layout alone and leaves the rules here as they are.
The fetch is the one Vault's, named or default, and never another bound Vault's.

What a rule set that cannot be read costs is a word on standard error and no rules for that run,
rather than failing a reading that has an answer: the reading is the thing being answered, and the
rules are a refinement of it.

### 2. [`app`] [`rola`] Speak every action to a Vault behind SSH, one ssh per action

Every command that speaks to a Vault now reaches one written as an scp destination. The client
was already one place — `proc_action` — so what changed is what happens there: instead of a
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
rather than from the moment its output ends: a session that failed before its first write — an
`ssh` that could not reach the host — is exactly the case with no output to read. A leg that cannot
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
accepts, or a window takes an address the command then refuses. It now knows the scp spelling too —
`user@host:path`, with the SSH port between the host and the path when one is named — and refuses
what the command refuses: no account, no host, no path, a port no session is made to, two colons
after the host, a path the far side would read as a flag.

What a failed check says is unchanged and is what a person needs for this one: the command line's
standard error, which carries what `ssh` said about the host key, the key, or the name that would
not resolve.

### 4. [`app`] [`RorolalaDesktop`] [`plugins`] [`rorolala-vcs`] Hide what a Workspace's ignore rules govern in the browser

The Rorolala plugin now contributes a hide rule to the File System plugin's catalogue, offered as
`rola_ignored` and labelled with the plugin's own key. A directory under the root being read is
looked up for the Workspace holding it, the Workspace's rule set is loaded once per Workspace
through the C ABI — off the thread the rows are drawn on, the way the provider contract asks — and
an entry the rules match is left out of a listing or a tree. Letting go of the answer when the files
may have changed is what makes an edited or freshly fetched rule file seen.

What the provider answers is the rules alone: an entry they match is hidden whether or not the
Layout records it, where the reading `rola status` makes keeps a recorded path out of the rules'
jurisdiction. That is the one place the browser and the reading can disagree, and it is the cost of
answering about a row without walking the tree the reading walks.

### 5. [`modules`] Manage ignore rules in one directory, and let a Vault hand rules down

Rorolala now reads its ignore rules from one managed directory — `.rola/info/ignore/` in a
Workspace, `./info/ignore/` in a Vault — instead of from the tree, and a fetch brings a Vault's
rules down with its Layout. A rule file is held under the name its author gave it and holds
gitignore's own syntax: negation, directory-only rules, anchoring and `**` all read as git reads
them. A file that the rules govern is not the work's gain, so a tree reading leaves it out of what
the tree gained; what a Layout already names is not the rules' to hide, and a move the reading
found is a fact about the work rather than something a rule suppresses.

The order the rules apply in is the managed directory's own answer: a deeper rule file is applied
before a shallower one, and beside its siblings a file whose name sorts sooner (numbers by value)
is applied before one that sorts later, so the last rule that matches a path is the one that
decides. A Vault's rules land under `vaults/<name>/` — deeper than the Workspace's own — which is
what lets a Workspace's `!*.mb` suppress a Vault's `*.mb`.

The rules themselves are matched by ripgrep's `ignore` crate rather than by a second
implementation of gitignore's language, and the set crosses the C ABI as an opaque handle
(`load_rola_ignore_rules`, `rola_ignore_rules_is_ignore`, `free_rola_ignore_rules`) so a program
outside Rust asks the same set the command line does.

`modules/tree-analyze` takes the rule set as part of a reading and filters only what the tree
gained, after the moves are paired. `modules/status` reads with it, so a status made of a Workspace
carries the same answer. `modules/daemon` gains the `fetch-ignore-rules` action (id 18), which
hands a Vault's whole rule directory over and replaces the Workspace's copy of it as one swap
— a rule the Vault deleted is gone from here after the next fetch, without either side keeping a
list of what used to be.

### 6. [`modules`] Reach a Vault over an SSH session, with no port bound at either end

A Vault can now be served to a caller that arrives through `ssh`, which binds no port and needs
nothing of the network beyond the `ssh` program the caller already has. The far side runs
`rola-daemon serve <PATH>`: one connection is carried by that process's own standard input and
output, and the process ends when the connection does. The table of namespaces is not read in that
mode — the path on the command line is the Vault, taken exactly as written — so a machine with
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

`modules/daemon` reads `rola/daemon/preference.toml` beside the namespace table — two switches,
both defaulting to the shape the feature was designed for, and written down with those defaults the
first time a session asks and finds no file there. A machine that has said nothing is a machine
whose switches a person still has to be able to find: the first session writes them, so turning one
off is editing a file rather than knowing where it would have been. A data directory that cannot be
written does not refuse the session — the defaults are what a machine that says nothing means — and
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
`[auth] admins` — an administrator may move any file in the Vault — and never registers anything
where the Vault's `key_discovery` does not name its own keys. Turning it on changes what revoking
a member means: a deleted key is written back by the next connection, so revoking is turning the
switch off or closing the SSH access.

The mode has two contracts that a caller depends on. The standard output carries the protocol and
nothing else, so a login script that prints breaks the session; everything the daemon has to say,
including the audit line of a registration, goes to the standard error, which `ssh` forwards to
the caller. And the exit code is the daemon's own — 4 for a path that is not a Vault, 5 for a failed
session, 6 for a machine that refuses sessions, 7 for preferences that will not read — because that
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

A build for a musl target makes one package instead, `rorolala-musl`: the same all-in-one, with the
command-line programs built against musl and the Desktop side — .NET, GTK and the C ABI's shared
library — the host's own, since those are glibc wherever they are built.

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
