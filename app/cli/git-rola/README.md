# git-rola

## Introduction

`git-rola` is the git side of Rorolala, the program git reaches for `git rola` because git runs `<name>-<word>` for a word it does not know. It keeps a project's `rola.toml`, `rola.lock` and working tree in step, places the materials the lock names, resolves a mapping's pin to the content a Vault serves, and installs the hooks that do the same after git changes the tree.

## Boundary

It owns the commands `init`, `hook`, `check`, `restore` and `update`, the hooks it installs for `post-checkout`, `post-merge` and `post-rewrite`, the wrapping of `checkout`, `switch`, `merge` and `pull`, and the exclude block that keeps materials out of git's view. It does not own the project format, the work tree, the exclude block or the staging area as rules, which are `rola-git-support`, nor the store and the exchange with a Vault, which are `librorolala`, nor serving a Vault, which is `rola-daemon`. It acts in the repository's main work tree only: a linked work tree is refused, since a repository keeps one exclude file for all of its work trees.
