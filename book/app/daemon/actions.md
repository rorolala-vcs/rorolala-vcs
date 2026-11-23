---
title: "Daemon Actions"
description: "The actions a Vault answers when a Workspace reaches it, what each one changes, and which command reaches it."
keywords: daemon, actions, protocol, handshake, sync, fetch, layout, ownership, versions, index, exchange
---

# Daemon Actions

An action is one thing a caller asks a Vault to do. The Workspace and the Vault run the same action: the side that holds a value sends it over the session when the other side needs it, both ends do the same step, and the result comes back. Which Vault an action runs against is the address's namespace, not a word in the action.

The number beside each action is its id. A caller outside Rust names an action by that id, and the daemon's build refuses two actions that claim one number. The numbers have gaps; a gap is not an action.

## At a glance

| ID | Action | What it does | Reached by |
| --- | --- | --- | --- |
| 0 | `handshake` | Exchanges the name the caller gives with the Vault's name and description. | `rola vault handshake` |
| 1 | `sync-all` | Makes the two stores hold the same keys, everything either side holds. | `rola storage sync-all` |
| 2 | `sync-index-all` | Makes the two version indexes hold the same objects. | `rola vcs-index sync-all`, and every fetch that needs an index |
| 6 | `ls-remote` | Lists what the other end's store holds, whole objects or cut contents. | `rola storage ls-remote-storaged`, `storage ls-remote-manifests` |
| 7 | `sync-hashes` | Makes the two stores hold the named keys. | `rola storage sync-hashes`, and fetching one object |
| 8 | `ls-remote-index` | Lists what the other end's index holds, variants, versions or text objects. | `rola vcs-index ls-remote-variants`, `ls-remote-versions`, `ls-remote-str` |
| 9 | `read-remote-index` | Reads one index object by hash, with a Version's number when there is one. | `rola vcs-index read-remote` |
| 10 | `fetch-layout` | Brings the Vault's Layout here whole, as a read-only copy. | `rola layout fetch`, and any command that needs the upstream Layout |
| 11 | `request-ownership` | Names the caller as holder of entries nobody else holds. | `rola hold`, `rola layout req-ownership` |
| 12 | `giveup-ownership` | Clears the holder from entries the caller holds. | `rola giveup`, `rola layout giveup-ownership` |
| 13 | `move-remote-path` | Moves where an entry sits in the Vault's Layout. | `rola layout path move` |
| 14 | `create-remote-entry` | Puts one `Uuid` into the Vault's Layout at a path. | The single-entry form of a sync write |
| 15 | `set-remote-version` | Moves the version the Vault's Layout holds for one entry. | The single-entry form of a sync write |
| 16 | `write-remote-entries` | Creates entries and sets versions in one exchange. | `rola sync` |
| 17 | `bring-remote-index` | Brings the chain under a version this index does not hold. | `rola retrack` |
| 18 | `fetch-ignore-rules` | Brings the Vault's ignore rules under the Workspace's own. | `rola layout fetch` |
| 19 | `remove-remote-path` | Takes an entry out of the Vault's Layout. | `rola layout path remove` |

## The greeting

`handshake` sends the name the caller gives and answers with the Vault's name and description. It is what `rola vault handshake` asks for, and what the launcher's address check reads: an answer here means the daemon is reachable, the namespace is registered, and the Vault proved who it is.

This is a greeting, not a session. Every exchange opens its own session, and the greeting is an action like any other rather than a step every command runs.

## Reading

These actions answer without changing what the Vault holds:

- `ls-remote` lists the store: the whole objects it holds, or the contents it keeps cut.
- `ls-remote-index` lists the index: the variants, the versions, or the Creator and Message text. A hash read here is one to hand to `read-remote-index`.
- `read-remote-index` reads one index object by hash. When the object is a Version, the number its chain traces to is worked out on the Vault, because the chain is there, and crosses beside the object.
- `fetch-layout` brings the Vault's Layout here whole. What arrives is a snapshot rather than a log, so a reader opens it without replaying anything. The copy is replaced as one generation, which is why a reader never sees half of a fetch. A copy that already matches the Vault answers as unchanged instead of crossing again.
- `fetch-ignore-rules` brings the Vault's ignore rules under the Workspace's own, in `info/ignore/vaults/<name>/`. A rule from a deeper directory yields to one written here, so the Workspace's own rules stand over the Vault's.

A fetch is not a sync: nothing travels the other way, and what arrives is read rather than worked in.

## Carrying content and index

These actions make both sides hold the same thing:

- `sync-all` asks for a full store sync. Each side says which keys it holds, and everything only one side holds crosses, in the Workspace's direction first and then the Vault's.
- `sync-hashes` is the same carrying narrowed to the keys the caller names. Fetching one object, or one set of objects, uses this rather than the full sync.
- `sync-index-all` does the same for the version index. An index object is one object and is never cut, so it crosses as its bytes, with none of the manifest and chunks a piece of content may need.

Nothing here is "upload" or "download". A run says what each side holds, and the difference travels. A key that names chunked content carries its manifest and then each chunk it names, so the receiving store keeps the chunks it was given rather than cutting the content again.

## The Vault's Layout

These actions change what the Vault holds, and every one of them is a change to the Vault's Layout rather than to the content or the index:

| Action | Who may ask | What changes |
| --- | --- | --- |
| `request-ownership` | Any account, for entries nobody holds | The holder becomes the account that asked. |
| `giveup-ownership` | The holder | The holder becomes nobody. An entry nobody holds is already what was asked for. |
| `move-remote-path` | Any account the Vault accepts a Layout write from | The path an entry sits at; the holder and the version stay. |
| `remove-remote-path` | The holder, or an administrator | The entry leaves the Layout. Its content and index objects stay. |
| `create-remote-entry` | Any account | A `Uuid` enters the Layout at a path, held by the account that asked. |
| `set-remote-version` | The holder, or an administrator | The version the entry is at; the path and the holder stay. |
| `write-remote-entries` | Any account, subject to the rules above for each write | Many creates and version changes, made as one batch. |

`create-remote-entry` and `set-remote-version` are the single-entry forms. `write-remote-entries` is the batch: it carries the entries to create and the versions to set together, and answers one outcome per write in the order they were asked for. `rola sync` uses the batch so that a run ahead on many files does not open a session per file.

Ownership moves in one run as well. `request-ownership` and `giveup-ownership` take the whole set of `Uuid`s over one session and answer one outcome per entry, which is what a command that names a directory wants.

## The chain under a version

`bring-remote-index` fills a gap rather than moving a pointer. A retrack can move a pointer to a version the Vault holds and this index never fetched, and such a version is readable only with the chain under it: the version, the variant that produced it, the words they are named by, and so on down to a version this index already holds.

The Workspace sends the hash it wants the chain of and everything it already holds, the Vault walks down until it meets something the Workspace has, and that part of the chain crosses back and is written here as one pack.

## Keeping this list true

This list states what the daemon registers. When an action is added, removed, or changes what it does, this article changes with it in the same step.
