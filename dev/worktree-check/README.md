# worktree-check

## Introduction

`worktree-check` reports how stale the local view is and what the worktree holds: the age of the
last recorded fetch, the `HEAD` branch, full ref and commit, staged, unstaged and untracked paths,
and the remotes, naming the one that points at the official repository. Run
`cargo worktree-check` before reviewing, rebasing or merging, so the branch and the changes you
read are the ones you think they are.

## Boundary

The tool reports, and `cargo worktree-check --fetch` runs `git fetch --all --prune` and records the
time of that fetch in `dev/worktree-check/__last_fetch`. It does not create branches, decide merge
policy, or change any file other than that timestamp.
