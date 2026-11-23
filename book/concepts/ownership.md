---
title: "About Ownership"
description: "How a Vault records who holds an entry, how a Workspace reads that record, and what holding and letting go do."
keywords: ownership, hold, giveup, owner, entry, lock, merge, variant, align, claim, accounts
---

# About Ownership

Ownership is how Rorolala says who is accountable for an entry. It is a recorded claim, not a lock on a file: holding a path does not stop anyone from editing it on disk, and the file is not read-only.

The record lives in the Vault's Layout. Each entry has an `owner`, which is an account name or nothing. The Vault's Layout is where that field changes, and a Workspace holds a read-only copy of it.

## How a Workspace reads it

Reading ownership is three lookups:

1. The path names a `Uuid` in the Workspace's own Layout.
2. That `Uuid` names an entry in the fetched copy of the Vault's Layout.
3. The entry's `owner` is the account that holds it.

The answer is therefore as old as the copy. A Workspace that has not fetched since someone else claimed an entry still reads the older answer. Nothing is reached for when ownership is read.

The reading has four outcomes:

| Outcome | What it means |
| --- | --- |
| Unnamed | The Layout names no entry at the path, the copy was never fetched, or the copy is older than the entry. |
| Free | The entry is there and no account holds it. |
| Mine | The account this run acts as holds it. |
| Held | Another account holds it, and its name is the answer. |

## Holding and letting go

Two commands move ownership, and each reads the Vault's Layout before it acts:

```sh
rola hold Assets/hero.psd
rola giveup Assets/hero.psd
```

`hold` claims an entry. `giveup` releases one. A name may stand for a directory, in which case it means every file under it, and `.` means the directory the run was made in.

The gates are read before any ownership moves:

- The Vault's current Layout is fetched, since every gate is about what the Vault holds now.
- A claim may be made over work that was written before the entry was locked. Letting go is stricter: the tree must agree with the version being handed over.
- A batch that cannot be done whole is not half done, unless `--allow-partial` says the doable part is worth doing.
- `--force` moves past the version check. It never moves past the holder.

After a claim or a release, the fetched copy is written with what the Vault agreed to, so the next reading shows the new state.

The Vault's own doors are `rola layout req-ownership` and `rola layout giveup-ownership`. They move the entry in the Vault's Layout and know nothing about the tree this side is standing on. The two commands above are the ones that read the work as well.

## Why ownership exists

Two people editing different files should not need to coordinate. Two people editing the same file should notice before they overwrite each other.

Ownership is the record that makes the second case visible. A path another account holds is reported as held, and a claim over it is refused. A path that is free can be claimed before work begins, so the next person to look sees who is working there.

Ownership is also what a merge is about. When another account has made a variant of a file, the variant arrives as a **variant file** beside the target, named after the target with the variant's short hash in it. A record under `.rola/MERGING` says which variant file belongs to which target. Joining the variant produces a merge variant, and that variant is what the next version is made from.

A variant file is not a path the Layout names. It is something waiting to become part of the work, so a reading of the tree is told to leave it out. If a merge record can no longer be carried out, because the target is gone or the variant is no longer held, the record stays where it is and a reading reports it rather than dropping it quietly.

## What ownership is not

Ownership is not a filesystem lock, a permission, or a key. It does not prevent an edit on disk, and it does not decide whether a session is allowed to speak. A session can be properly authenticated and still be refused a claim, because the holder is someone else.

It is also not the version. Holding an entry says who is accountable for it, not which version it is at. The two are separate fields of the same entry, and a claim can be made about an entry whose content has moved on.
