---
name: plan
description: Draft and iterate an implementation plan under `__PLAN_<name>/README.md` without touching code. The README frontmatter records `base-commit`, `date` and `work-on`, and the body is numbered steps a fresh session can run without asking anything. Use when the user asks for a plan, a draft, a design or a restructuring proposal, or asks for a change that must not touch code.
---

# Plan

A plan is a contract another session executes with the `work` skill. This skill writes that contract
and nothing else: no source edit, no build, no test run, no branch switch, no commit.

## When to use it

Load this skill when the user asks for a plan, a draft, a design or a restructuring proposal, or when
the user asks for a change that must stay out of the code. The trigger is the intent, not a fixed
phrase: any request shaped as "decide first, write later" belongs here.

## Shell variants

Most commands here are `git` or `cargo` calls, which take the same arguments in every shell. The two
that change follow. In `pwsh` 7 and Windows PowerShell 5.1, `$?` is a boolean; the exit code of a
native command is `$LASTEXITCODE`.

| Need             | `sh`             | `pwsh` 7                                      | Windows PowerShell 5.1                        |
| ---------------- | ---------------- | --------------------------------------------- | --------------------------------------------- |
| Today's date     | `date +%F`       | `Get-Date -Format 'yyyy-MM-dd'`               | `Get-Date -Format 'yyyy-MM-dd'`               |
| Plan directories | `ls -d __PLAN_*` | `Get-ChildItem -Directory -Filter '__PLAN_*'` | `Get-ChildItem -Directory -Filter '__PLAN_*'` |

`git rev-parse`, `git add -f` and the rest of the `git` calls in this skill are written once and work
in all three shells.

## Non-negotiables

- **Do not edit code.** A plan names the change; it does not make it.
- **Do not invent the interface.** Commands, flags, options, exit codes and user-facing text belong to
  the user. Ask, then write the answer down.
- **Do not leave an open question in a finished plan.** Every choice is decided or listed under
  `## Open questions`, and the plan is not ready while that list is non-empty.
- **Write down what the user decided.** When the wording is the contract, quote it.
- **Ground every claim.** A file, symbol or line named in a step exists in the tree you read. Open it
  before citing it.

## Before the first draft

1. Restate the task in one or two lines: the goal, the area, and that the plan is the only output.
2. Run `cargo worktree-check` for the branch and the short hash. Add `--fetch` when the plan will
   branch from `main`, so `base-commit` is the hash the user will see next.
3. Search the official repository for an existing issue, as the `worktree-check` skill describes.
4. Look for an existing plan: `ls -d __PLAN_*` in the project root. When one covers the same subject,
   read it and extend or replace it with the user's agreement. Two plans for one change will drift.
5. Load `project-orientation` when the area is new to you, and read the READMEs of the crates the plan
   touches.
6. Ask what the request leaves open before drafting: the goal, the accepted breakage, the branch name
   when it matters. Each answer goes into the plan, not into your memory of the conversation.

## Where the plan lives

`__PLAN_<name>/README.md` in the project root, with `<name>` a short kebab-case subject such as
`__PLAN_chunking-registry`. Supporting files may sit beside it (`NOTES.md`, a sketch, a measured
table). `README.md` is the entry and must stand alone.

`__*` is gitignored, so a plan is a local document. Say so when the user expects it in the
repository; adding it takes `git add -f`.

## The frontmatter

Every plan starts with these three keys and nothing above them:

```markdown
---
base-commit: 4f3a9c1
date: 2026-11-20
work-on: wip/chunking-registry
---
```

- `base-commit`: `git rev-parse --short=7 HEAD`, seven characters, on the tree the plan was written
  against.
- `date`: `date +%F`, the day the plan first took shape. It stays on every revision; the revision adds
  a line to `## Revisions`.
- `work-on`: the branch the change is implemented on, `wip/<name>` for work and `fix/<name>` for a fix,
  off `main`. The `work` skill refuses to start without it.

A revision against a newer tree moves `base-commit` to the new hash and records the date, the hash and
the change in `## Revisions`. A stale `base-commit` makes `work` report drift that was already settled.

## README shape

```markdown
---
base-commit: 4f3a9c1
date: 2026-11-20
work-on: wip/chunking-registry
---

# <Title in the user's words>

## Goal

One paragraph. What is true after the work that is not true now.

## Decisions

| Decision  | Value                    | Why |
| --------- | ------------------------ | --- |
| Command   | `rola chunk ...`         | ... |
| Exit code | `ErrorChunkBroken = 261` | ... |

## Steps

### S1. <verb phrase>

- **Files**: `modules/chunking/src/registry.rs:120`, `src/exit_codes.rs`
- **Change**: what the code says now, and what it says after.
- **Verify**: `cargo ci test -p chunking`
- **Status**: pending

### S2. ...

## Verification

The `cargo ci check` steps this change reaches, in order, and why the others do not.

## Out of scope

## Revisions

| Date | Base | What changed |
| ---- | ---- | ------------ |

## Open questions

Empty when the plan is ready.
```

## A step that can be executed

A session holding the repository and this README, with no conversation, runs every step. A step is
ready when:

- it names every file it touches, with a path and the symbol or section inside it;
- it states the change in terms of the code that is there now, not in terms of an intended design;
- it gives the command that proves the step, or names the test that must pass;
- it sits after the steps it depends on, and says so when the order matters;
- it names any tool, fixture or corpus it needs, and whether that is shipped.

`**Status**: pending` becomes `done <YYYY-MM-DD>` when `work` runs the step. Nothing else in the step
changes during execution.

## User-facing text

When the plan adds or changes a string a user reads, write the final text for both `en` and `zh-CN` in
the plan, and check every new sentence with
`.agents/skills/write-documents/scripts/check_symbols.py`. Implementation then copies the plan's text
instead of inventing it. Help text goes through the `cli-help-check` skill once it is written.

## Iterating with the user

Each round:

1. Restate what the plan now says in a few lines: goal, decisions, step count, open questions.
2. Ask only the questions that change the plan, with the choices and their cost. Do not ask what the
   tree already answers.
3. Update the README in the same round, so the file and the conversation never disagree.
4. Say what is still missing before the plan is an SOP.

Iterate until the done test passes, then say the plan is ready and what `work` will do first.

## The done test

The plan is ready when a session that reads only `__PLAN_<name>/README.md` can:

- name the branch it works on, from the frontmatter;
- run every step in order without a question;
- tell whether each step succeeded;
- know what not to touch;
- find `## Open questions` empty.

Then stop. Implementation belongs to `work`.

## Reporting

Tell the user the plan path, the goal in one line, the number of steps, the branch in `work-on`, and
that nothing was edited. List any question that remains and what it changes.
