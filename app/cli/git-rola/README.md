# git-rola

## Introduction

`git-rola` is the git side of Rorolala, the program git reaches for `git rola` because git runs `<name>-<word>` for a word it does not know. It keeps a project's `rola.toml`, `rola.lock` and working tree in step, places the materials the lock names, resolves a mapping's pin to the content a Vault serves, and installs the hooks that do the same after git changes the tree.

## Boundary

It owns the commands `init`, `hook`, `check`, `entries`, `bring`, `restore`, `refresh`, `update`, `help` and `version`, the hooks it installs for `post-checkout`, `post-merge` and `post-rewrite`, the wrapping of `checkout`, `switch`, `merge` and `pull`, and the exclude block that keeps materials out of git's view. It signs a Vault's entries in to a project (`bring`, which writes the mapping and the lock entry it resolved), lists what the lock names (`entries`), and applies the lock without reaching a Vault (`refresh`). It speaks the way git speaks: its streams, statuses, message prefixes, usage text, pager choice and language are git's rules, so `git rola` differs from a git command only in its subject. It does not own the project format, the work tree, the exclude block or the staging area as rules, which are `rola-git-support`, nor the store and the exchange with a Vault, which are `librorolala`, nor serving a Vault, which is `rola-daemon`. It acts in the repository's main work tree only: a linked work tree is refused, since a repository keeps one exclude file for all of its work trees. It writes no man page and no HTML documentation: `--help` answers with the usage text, and `git help rola` remains git's own documentation lookup.
