# rola-define-format

## Introduction

The `rola.lock` format: the materials a project places, and the Vault sources their content can be fetched from.

## Boundary

It owns the grammar, the one canonical spelling, the format version, and the hash, name and path types a lock names. It is the only place a lock is read or written. It does not read or write files, reach a network or touch git: `modules/git-support` reads the lock and places the materials, and `rola.toml`, the file a person writes, is a different format owned elsewhere.
