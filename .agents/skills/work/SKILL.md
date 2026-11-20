---
name: work
description: Execute an existing plan from `__PLAN_<name>/README.md` end to end. Confirms which plan with the user, checks that the `work-on` branch exists and that `base-commit` still matches the starting commit, reports drift in the files and interfaces the plan names, then implements the steps and runs the plan's checks. Use only when the user asks to start the work, implement a plan or carry out a named plan, and confirms which plan.
---

# Work

Execute a plan. This skill is where a plan becomes code, and it starts only after the three
confirmations below are answered.

## When to start

Load this skill only when the user asks to execute a plan: to start the work, to implement the plan
just drafted, or to carry out a named plan. The wording varies and the intent does not. A plan that
exists is not an instruction to run it, and a question about a plan is not one either. When in doubt,
ask before reading the plan for execution.

## Shell variants

`git` and `cargo` take the same arguments in every shell. The blocks that differ are given twice, and
the notes below cover the rest. In `pwsh` 7 and Windows PowerShell 5.1, the exit code of a native
command is `$LASTEXITCODE`; `$?` is a boolean there. Windows PowerShell 5.1 has no `&&`, so run
chained lines one at a time. Quote an argument that contains braces, because `{` opens a script block
in PowerShell.

## Step 1: identify the plan

When the user did not name one, or the name is vague, list the plans and ask:

```sh
ls -d __PLAN_*
```

```powershell
Get-ChildItem -Directory -Filter '__PLAN_*'
```

Read each frontmatter and show one line per plan:

```text
__PLAN_chunking-registry    base 4f3a9c1 (2026-11-20)    work-on wip/chunking-registry    open questions: none
```

Ask which one, with the choices and their cost. Do not pick the newest, or the only one, on your own.

## Step 2: read the plan and restate it

Read the whole README and every file it points at. Restate in your own words: the goal, the decisions,
the steps in order, the branch, and the verification. List anything you would have to decide to
execute it; each one is a reason to stop before the first edit.

## Step 3: check the worktree

```sh
cargo worktree-check --fetch
```

- Confirm the official remote.
- Read the staged, unstaged and untracked paths. Changes that mix with this plan stop the run; ask
  what to do with them first.
- A detached `HEAD` stops the run, because the plan names a branch to be on.

## Step 4: settle the branch

`work-on` is never optional. Resolve the starting point, and get the user's answer where a choice
exists:

```sh
git --no-pager branch --show-current
git --no-pager rev-parse --verify --quiet refs/heads/<work-on>
```

- **Already on `work-on`**: say so, show `git --no-pager log --oneline main..<work-on>` when it has
  commits, and continue.
- **`work-on` exists and another branch is checked out**: show its commits and ask whether to switch
  to `work-on` or to implement on the current branch. Staying makes the frontmatter stale, so the
  plan must record the branch actually used.
- **`work-on` does not exist**: say which branch will be created and from where, and get an answer
  before creating it. The project rule is a branch off `main`, after Step 5 passes:

  ```sh
  git switch main
  git merge --ff-only <official>/main
  git switch -c <work-on>
  ```

## Step 5: compare `base-commit` with the starting commit

Run the checks against the point the work will start from: `HEAD` when staying where you are, the
`work-on` tip when switching, or `main` after the fast-forward when creating.

```sh
git --no-pager rev-parse --verify --quiet <base-commit>^{commit}
git --no-pager merge-base --is-ancestor <base-commit> <start>; echo $?
git --no-pager log --oneline <base-commit>..<start>
git --no-pager diff --name-status <base-commit>..<start>
```

```powershell
git --no-pager rev-parse --verify --quiet '<base-commit>^{commit}'
git --no-pager merge-base --is-ancestor <base-commit> <start>
$LASTEXITCODE
git --no-pager log --oneline <base-commit>..<start>
git --no-pager diff --name-status <base-commit>..<start>
```

`--is-ancestor` exits 0 when `base-commit` is in the history, 1 when it is not, 128 when the hash is
unknown.

Classify the drift:

- **none**: no commit between the two, or the diff touches nothing the plan names.
- **cosmetic**: the touched files are outside the plan's scope. Re-read the plan against the current
  tree, record the result, and go on.
- **material**: the diff touches a file a step edits, a symbol a step calls, the exit-code block the
  plan allocates from in `src/exit_codes.rs`, an i18n key the plan adds, the generated header or
  `src/exit_codes/explain.rs`, or `[workspace.dependencies]`.
- **ahead**: `base-commit` is not in the start point's history. The plan may name code that does not
  exist here. Treat it as material.

Material or ahead drift stops the run. Report the commits and the paths, and ask whether to load the
`plan` skill and revise the plan, or to change the step now with that decision recorded.

## Step 6: implement

- Restate the next step before editing.
- Stay inside the files the step names. A file outside the list stops the run, because the plan is the
  scope contract.
- A step that cannot be executed as written stops the run. Do not improvise a plan change into the
  code.
- Keep the project's rules: a subcommand follows the `#[command]` and `#[chain]` shape, global flags
  are consumed rather than declared, an exit code gets a constant, an `exit_codes.<key>` doc comment,
  both i18n files and a regenerated `src/exit_codes/explain.rs`, user-facing copy is checked with
  `.agents/skills/write-documents/scripts/check_symbols.py`, a shipped change gets `cargo changelog`
  through the `changelog-entry` skill, and an FFI change is checked with a C compiler against the
  generated header.
- Flip `**Status**: pending` to `**Status**: done <YYYY-MM-DD>` in the plan. Never rewrite the steps or
  the frontmatter.

## Step 7: verify and rebuild

- Run the plan's `## Verification`. When it lists none, run the gate steps the change reaches.
- The gate signals through its exit code:

  ```sh
  cargo ci check > /tmp/log 2>&1; echo $?
  ```

  ```powershell
  cargo ci check *> "$env:TEMP/rola-ci.log"
  $LASTEXITCODE
  ```

  Windows PowerShell 5.1 writes that log as UTF-16, so read it with `Get-Content`. On a `pwsh` 7
  running outside Windows, `$env:TEMP` may be empty; use `/tmp/log` there.

- Rebuild the artifacts with `cargo dev export` at the end of the round, unless the user says not to.
- Do not commit. The user reviews, runs CI and commits.

## Step 8: report and hand back

Give the plan path, the steps done and remaining, the branch, the files changed, each check with its
exit status, and that the artifacts were rebuilt. When the plan is complete, say so and leave the
commit to the user.

## Stop conditions

Stop and ask when:

- no plan is named, or the user has not asked to implement one;
- the tree holds changes that would mix with this work;
- `HEAD` is detached;
- `base-commit` is unknown, or is not in the history of the starting commit;
- the drift is material;
- a step cannot be executed as written, or names a missing file or symbol;
- a check fails for a reason outside the plan.

## When the plan is wrong

A cosmetic correction inside a step, such as a renamed local variable, is part of the work. Anything
that changes a file list, an interface, an exit code, an i18n key or a verification command changes the
plan, and goes back through `plan`. Record what you found and ask; do not patch the plan with a new
decision of your own.
