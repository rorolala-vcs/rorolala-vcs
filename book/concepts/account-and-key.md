---
title: "About Accounts and Keys"
description: "What an account is, what a member is, how a key proves an identity, and what a handshake with a Vault protects."
keywords: account, member, key, ed25519, fingerprint, identity, handshake, session, admins, auth
---

# About Accounts and Keys

Rorolala identifies a person by a key, not by a name. A name is a local label for a file, and the key inside it is what says which identity an action runs as.

## Account and member

The same key pair appears twice, under two names:

- An **account** is the private key. Its name is the stem of the `.pem` file, and the file is what the account proves itself with. An account is never shared, so it is looked for only beside the work at hand: in a Workspace or a Vault, never in a scope a public key could come from.
- A **member** is the public key. Its name is the stem of the `.pub` file, and the file is meant to be shared. A member is found in any scope a key search covers, and a `.pub` that no Vault has made a member of yet is not an error.

Both are read-only views of a file. A key is read afresh each time it is needed, so an account is the file it names rather than a copy taken when the program started.

## The pair beside the account

A key generator writes a `.pem` and a `.pub` with the same stem. That pair means one of two things:

- The `.pub` holds the public half of the account's own `.pem`. It is the pair's public side, and it says nothing about anyone else.
- The `.pub` holds a different key. Then it is the peer key, and a client holding the account holds its peer to it: the peer must prove that key before anything runs.

The difference is decided by comparing the two, not by where the file sits. An account without a `.pub` can still prove itself when challenged, but it has nothing to check the peer's answer against, so it is only challenged rather than challenging back.

## Fingerprints

A fingerprint is the identity. It is the SHA-256 of the key algorithm's name, a zero separator, and the public key itself.

Two peers may each call the same key something different, so a member table is looked up by fingerprint rather than by name. A name is only how one machine spells a file.

## Choosing the account

```sh
rola key generate alice --install
rola account alice
```

`rola key generate` makes a pair. `--install` writes it into the user's key directory rather than beside the current work. `rola account <name>` records which account the work acts as, and `rola account current` prints it. `rola account ls` lists the accounts a run can see.

To list keys without choosing one, use `rola key` for public keys and `rola key --pem` for private ones.

## What a Vault knows

A Vault keeps its own key pair under `keys/`, and it recognises the accounts it lets in by their public keys. The administrators are names in the Vault's `vault.toml`, under `[auth]`; a name there is the name of a `.pub` file, so an administrator is a member the Vault lets administer it.

A private key never travels to a Vault. What crosses is a signature made with it, and the public key the signature is checked against.

## The handshake

When a Workspace reaches a Vault, the socket becomes a session through a signed handshake:

- The exchange is Diffie-Hellman, signed rather than implicit. Each side proves its identity by signing the transcript, and the other side checks the signature with the public key it holds. The signature algorithm is Ed25519.
- The exchange is hybrid. It mixes X25519 for forward secrecy now with ML-KEM-768 for forward secrecy against a key recovered later, so a session recorded today is not opened by a future key recovery.
- What follows is a record layer over the same socket: length-prefixed, sealed records with ChaCha20-Poly1305, counters and rekeying, so the protocol above reads and writes a plain stream.

This is what `rola vault handshake <address>` performs, and what the launcher's address check reports on. A refusal here is a refusal of identity or of reachability, not of permission to read a particular file.

## What ownership does not change

Ownership and keys are separate questions. A key says who is speaking. Ownership says who holds a path in the Vault's Layout, see `concepts/ownership`. A session can be perfectly authenticated and still be refused a claim, because the holder of the entry is someone else.
