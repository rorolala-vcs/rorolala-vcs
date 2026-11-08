---
name: worktree-check
description: Check the git worktree before writing code. Reports the age of the last recorded fetch, the HEAD branch and commit, whether it is detached, staged, unstaged and untracked paths, and the remotes, naming the one at github.com/rorolala-vcs/rorolala-vcs. Also states the branch, issue-search and linear-merge rules. Use before the first edit, before creating a branch, or when the user asks about the repository state.
---

# Worktree Check

Run this before reading code for a change, and before the first edit. It tells you whether the
branch you are about to work on is the one you think, whether the tree already holds changes, and
how stale the view of the remote is.

## Commands

```sh
cargo worktree-check
cargo worktree-check --fetch
```

`cargo worktree-check` only reports. `cargo worktree-check --fetch` runs
`git fetch --all --prune`, records the time of a successful fetch in
`dev/worktree-check/__last_fetch`, then reports. The file is ignored by the project's `__*` rule.

## What the report says

- `last fetch`: how long ago the last recorded fetch happened. `never recorded` means the timestamp
  has not been written yet; run the command with `--fetch`.
- `head`, `ref` and `commit`: the branch, the full ref it names, and the short hash. `detached: yes`
  means no branch is checked out, so a commit made now would not move a branch.
- `staged changes`, `unstaged changes`, `untracked paths`: the `git status --porcelain` lines,
  grouped. Read them before you start so you do not mistake another change for yours.
- `remotes` and `official remote`: the remote whose URL points at
  `github.com/rorolala-vcs/rorolala-vcs`. If it says `none`, stop and ask which remote is the
  official one before fetching from a guess.

## Branch rule

Work off `main` on a branch named for the task:

```sh
cargo worktree-check --fetch
git switch main
git merge --ff-only origin/main
git switch -c wip/<name>
```

Use `fix/<name>` when the task is a fix. Replace `origin` with the official remote the report
names. `git merge --ff-only` keeps `main` linear; a plain `git merge` would create a merge commit.

Do not commit unless the user asks. The user reviews, runs CI and commits.

## Look for an issue first

Before writing a feature or a fix, check whether the official repository already has an issue for
it. Prefer `gh`:

```sh
gh issue list --repo rorolala-vcs/rorolala-vcs --state all --search "<terms>"
```

When `gh` is not installed, use `curl` or the agent's fetch tool:

```sh
curl -sS "https://api.github.com/search/issues?q=repo:rorolala-vcs/rorolala-vcs+<terms>"
```

URL-encode a space as `+`. Read the issue before deciding the scope; an existing report may already
name the shape the user expects.

## Merging, only when the user asks

Keep `main` linear. Rebase the branch, then fast-forward:

```sh
git switch wip/<name>
git rebase main
git switch main
git merge --ff-only wip/<name>
```

When the user wants one commit for the branch, squash instead:

```sh
git switch main
git merge --squash wip/<name>
git commit
```

A merge without `--ff-only` or `--squash` is refused by this project's rule. After either path,
confirm that no merge commit was made:

```sh
git --no-pager log --merges --oneline main
```

The command should show nothing new for this work.
