---
title: "Glossary"
description: "The words Rorolala uses, one line each, with the article that explains the one you need."
keywords: glossary, terms, definitions, account, vault, workspace, layout, store, variant, version, ownership
---

# Glossary

The words below are used across the command line, the Desktop program and this book. Each line says what the word means and where to read more.

| Term | What it means |
| --- | --- |
| Account | The private key a run acts as. Its name is the stem of the `.pem` file. See `concepts/account-and-key`. |
| Address | The host, optional port and namespace that name a Vault a daemon serves. See `concepts/address`. |
| Bring | Putting an entry the Vault holds at a path here, or joining a variant into a file. See `concepts/layout`. |
| Chunk | One piece of stored content. Content that is chunked is recorded by a manifest. See `vcs-knowledge/chunk`. |
| Creator | The name a variant carries to say who made it. It is a text object of at most 256 bytes. See `vcs-knowledge/variant`. |
| Daemon | `rola-daemon`, the program that serves registered Vaults, one namespace each, over TCP or SSH standard streams. See `concepts/daemon`. |
| Entry | What a Layout keeps for one `Uuid`: its kind, owner, version, description and chunking preference. See `concepts/layout`. |
| Fingerprint | The SHA-256 of a key algorithm's name, a separator and the public key. It is what identifies an identity. See `concepts/account-and-key`. |
| Key | An Ed25519 identity pair. The `.pem` is private and proves the identity; the `.pub` is public and is how the identity is known. |
| Layout | The record of which path names which `Uuid`, and what is kept for each. A Workspace and a Vault each keep one. See `concepts/layout`. |
| Manifest | The record of the chunks a piece of content is made of. The content's key does not cover the manifest. See `vcs-knowledge/manifest`. |
| Member | A public key file. Its name is the stem of the `.pub`, and it may be found in any scope a key search covers. See `concepts/account-and-key`. |
| Message | The short text a variant carries to say what was done. It is a text object of at most 256 bytes. See `vcs-knowledge/variant`. |
| Namespace | The name a daemon serves one Vault under. See `concepts/daemon`. |
| Ownership | The `owner` field of a Vault Layout entry. It is a recorded claim, not a filesystem lock. See `concepts/ownership`. |
| Pack | One file holding many stored entries, with an index beside it that records each key's offset and length. See `vcs-knowledge/storage`. |
| Registry | The daemon's table of namespaces and the Vaults they name. The address book in the launcher is read from it. |
| Store | A content-addressed directory, made one by its `rolast.toml`. See `vcs-knowledge/storage`. |
| Track | Recording content in the store, making a version of it, and renewing the Layout entry that names it. See `concepts/layout`. |
| Tree | A directory versioned as one object. A Layout entry of kind Tree names one. See `concepts/layout`. |
| Uuid | The identity of an entry. A path names one, and the entry is kept under it. See `concepts/layout`. |
| Variant | An edit that has not been fixed as a version. It points at content, a base version, a creator and a message. See `vcs-knowledge/variant`. |
| Vault | The server-side directory a Workspace sends work to, made one by `vault.toml`. See `concepts/vault`. |
| Version | An editing state that has been fixed. It points at one variant and is numbered by the chain it sits in. See `vcs-knowledge/version`. |
| Workspace | The directory a person works in, made one by `.rola/`. See `concepts/workspace`. |

## Words that look alike

- A **store** holds content by key. A **Vault** holds a store, an index and a Layout. A **Workspace** holds a store, an index and a Layout too, and the difference is which side it is on.
- A **variant** is an edit. A **version** is that edit fixed. A version points at a variant, not the other way around.
- **Ownership** says who is accountable for an entry. A **key** says who is speaking. They answer different questions.
- A **Layout** says what the work is. The **working tree** is what the filesystem holds. `rola status` is the reading that compares them.
