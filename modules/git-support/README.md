# rola-git-support

## Introduction

The local half of rola's git support: reading `rola.toml` and `rola.lock`, finding the project's git work tree, and moving materials between the working tree and the project's private area.

## Boundary

It owns reading the project files, the project's block of `.git/info/exclude`, the staging area under `.rola/git/`, the state of what was last placed, and the refusal of a linked work tree. It does not write `rola.lock`: the text of that file is `modules/define-format`'s. It does not fetch anything either: the network, the Vault and the store belong to the `git-rola` programs, which call this crate and answer on their own.
