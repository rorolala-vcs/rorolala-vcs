---
name: project-quality-review
description: Read-only quality audit for rorolala's Rust tree. Inventories Clippy and rustc allow/expect attributes and escape hatches (unsafe, unwrap, expect, panic macros), flags missing, vague, over-broad or clearly unnecessary justifications, and writes timestamped `__QR_CLIPPY_ALLOW.md` and `__QR_WORKAROUND.md` reports after backing up previous runs. Never edits code and never runs builds, tests or Clippy. Use when the user asks for a workaround, escape-hatch, lint-allow or project quality review.
---

# Project Quality Review

An audit of the bypasses in the Rust tree: what is suppressed, what is unchecked, and whether the
reason written beside it actually holds. It is **read-only**. The only files it writes are the two
reports in the project root and the backups of older reports. No source, test, manifest or
configuration is edited; no build, test, formatter or Clippy run is started.

The audit answers two questions:

1. **Lint suppressions** — every `allow`/`expect` on a Rust lint: is there a reason, is it specific,
   is the scope the narrowest that works, and is the suppression obviously unnecessary?
2. **Escape hatches** — every `unsafe`, `unwrap`, `expect` and panic path: is it marked with the
   project's `// SAFETY:`, `// UNWRAP:` or `// PANIC:` comment, and does that comment ground the
   claim?

## Non-negotiables

- **Never edit code.** If a finding needs a fix, describe it in the report. Fixing is a separate task.
- **Never run** `cargo`, `cargo clippy`, `cargo test`, `cargo ci`, a formatter, or a test binary. CI
  already reports whether tests pass; this audit is textual. Shell use is limited to read-only text
  search (`rg`/`grep`), `date`, and a read-only `git status` for verification.
- **Never invent** a path, line, lint, snippet or finding. Open the cited line before writing it down.
  If a line moved, correct it; if a claim cannot be grounded in the text, mark it `unverified` rather
  than asserting it.
- **Never widen the scope silently.** Audit what the user named; if nothing was named, audit the Rust
  roots listed below.
- **Reports are snapshots, not verdicts.** A justified bypass stays in the inventory; it is not
  erased from the record.
- **Report prose follows the user's language.** Keep identifiers, paths, lint names and snippets
  verbatim. The skill itself is English.

## Before the sweep

1. **Restate** the audit in one or two lines: the roots, the two checks, the two output files, and the
   read-only rule. Do this before reading files for findings.
2. **Fix the scope.** If the request names a crate, directory or file, audit only that and say so in
   the report. If it names nothing, audit every Rust root: `src/`, `app/`, `dev/`, `modules/`,
   `utils/`, `tests/`. Never silently include or exclude an area.
3. **Get the timestamp** for the titles:

   ```sh
   date -Iseconds
   ```

   If that is unavailable, use `date`; if neither works, use the date the environment reports and say
   the time is unknown in the report. Never invent a time.

4. **Look for existing reports.** `__QR_CLIPPY_ALLOW.md` and `__QR_WORKAROUND.md` in the project root.
   They are backed up immediately before the new report is written (see "Writing the reports").
5. **Decide on subagents** from the tools actually available (see "Subagents"). Do not claim an agent
   was used when it was not.

## What is in scope

Rust text (`.rs`) under the roots chosen above. Exclude:

- `target/`, `.cache/`, `THIRD-PARTY/`, `vendor/` and any dependency source outside the workspace.
- Generated Rust. Known generated files are `src/exit_codes/explain.rs`,
  `modules/chunking/src/policies/mod.rs` and `modules/chunking/src/registry.rs`; check the headers and
  the build scripts rather than assuming this list is complete. A problem in a generated file is
  reported against the source or build script that produces it.
- C#, C, the generated C header, and TOML/YAML/Markdown configuration: the checks are Rust-specific.
- Doc-test snippets and macro expansions as separate findings. Report at the macro definition.

`#[cfg(test)]` code and files under `tests/` are read and counted, but classified `test-only` (see
"Test code"). The `tests/` tree is a workspace of its own and builds real programs; it is not skipped,
because `unsafe` there is still `unsafe`.

## Finding the events

Use the editor's search or `rg`/`grep`; these patterns are the important part, not the tool. Read the
comment block above every hit and the enclosing item: a hit line alone is not enough to classify.

```text
# attributes; multi-line attributes need the following lines read too
#!?\[(allow|expect)\s*\(
cfg_attr\([^)]*(allow|expect)\s*\(

# unsafe — the word also occurs in comments and docs, so inspect each hit
\bunsafe\b

# unwrapping
\.unwrap\(\)
\.unwrap_err\(\)
\.expect\(
\.expect_err\(
\.unwrap_unchecked\(

# panic paths
\b(panic|unreachable|todo|unimplemented)!\s*[\(\[\{]
\bassert(_eq|_ne)?!\s*[\(\[\{]
\bdebug_assert(_eq|_ne)?!\s*[\(\[\{]

# workaround markers
//\s*WORKAROUND:
```

`.unwrap_or` does not match `\.unwrap\(\)`, which is deliberate: it is not a panic path.

## Check 1 — Lint suppressions (`__QR_CLIPPY_ALLOW.md`)

Find every:

- `#[allow(...)]`, `#![allow(...)]`
- `#[expect(...)]`, `#![expect(...)]`
- the same inside `#[cfg_attr(..., allow(...))]` / `cfg_attr(..., expect(...))`

Include rustc lints and lint groups, not only `clippy::*`; label each entry `clippy` or `rustc`. A
crate-root `#![deny(...)]` is not an allow and is not reported.

For each entry record:

- **Location** — path and the first line of the attribute.
- **Scope** — crate (`#!`), module, item, macro body, test-only.
- **Lints** — every lint in the attribute, and whether it is `clippy::` or rustc.
- **Justification** — `none`, `reason = "..."`, a comment block, or a doc comment.
- **Status** — see below.
- **Severity** — see the rubric.

### What counts as a justification

A justification must be one of:

- a comment block that ends on the line directly above the attribute, with no blank line between them;
- the attribute's own `reason = "..."`.

It must say why the lint's advice is wrong **here**. A comment that only describes the item, restates
the lint's name, or says "needed" is not a justification. A doc comment counts only if it explains the
suppression rather than the item's general purpose. Inside a `macro_rules!` body, a comment above the
macro definition that addresses the allow also counts. When a comment and a `reason` both exist and
disagree, report the conflict.

**Vague (flag):** "needed", "required", "clippy is wrong", "false positive", "temporary", "for now",
"TODO", "FIXME", "legacy", "because it complains", "for compatibility" with nothing named, or a
restatement of the lint name. A reason that names the constraint, the generated code, the framework
injection, or the signature it must match is specific enough.

### Statuses

- **justified** — a specific reason that holds on the evidence in the text.
- **missing** — no comment and no `reason`; always a finding.
- **vague** — a comment or `reason` exists but does not ground the suppression.
- **over-broad** — a module- or crate-level allow for a lint only one item needs. Suggest the
  narrowest scope (do not apply it). Exception: `#[chain]` copies a handler's attributes onto the
  struct it generates, so a module-level allow can be the only place it can live; if the comment says
  so, do not flag the scope.
- **unnecessary** — the text proves the lint cannot fire on the annotated item, or the allow is dead.
  Examples: `too_many_arguments` on a two-parameter function; `struct_excessive_bools` on a struct
  with fewer than three bools; `unused_self` on a free function; a `missing_const_for_fn` allow on a
  non-function item.
- **duplicate** — the same lint is allowed at an enclosing scope and again on the item, with no
  `#[cfg]` or macro boundary between them. Cite both locations.
- **test-only** — inside `#[cfg(test)]` or `tests/`; still recorded, severity capped as below.

Do not claim a lint "does not fire" merely because the trigger is not obvious. If the text cannot
prove it, mark `unverified` in the note rather than creating an `unnecessary` finding.

## Check 2 — Escape hatches (`__QR_WORKAROUND.md`)

Inventory every occurrence:

- **Unsafe:** `unsafe { ... }`, `unsafe fn`, `unsafe impl`, `unsafe trait`, `unsafe extern`, and
  `#[unsafe(...)]` attributes.
- **Unchecked unwrapping:** `.unwrap()`, `.unwrap_err()`, `.expect(...)`, `.expect_err(...)`,
  `.unwrap_unchecked(...)` (this one is also `unsafe`).
- **Panic paths:** `panic!`, `unreachable!`, `todo!`, `unimplemented!`, and `assert!`, `assert_eq!`,
  `assert_ne!`, `debug_assert!`, `debug_assert_eq!`, `debug_assert_ne!` outside test code. A manual
  `if !condition { panic!(...) }` is a panic path; the comment belongs above the `panic!`, or above
  the `if` when the whole block exists to panic.
- **Workaround markers:** `// WORKAROUND:` (and `# WORKAROUND:` if C-style comments appear). These are
  informational; each must say what is worked around and when it can go.

Not findings: `.unwrap_or(...)`, `.unwrap_or_else(...)`, `.unwrap_or_default(...)`, a custom method
whose name merely starts with `unwrap`/`expect` (this repo has `expect_message`), `?`, `Option`/`Result`
pattern matching, and `is_some`/`is_ok`.

### Required markings

| Construct                                                           | Marking      | What it must say                                                                |
| ------------------------------------------------------------------- | ------------ | ------------------------------------------------------------------------------- |
| `unsafe`                                                            | `// SAFETY:` | the invariant the operation requires, why it holds, and where it is established |
| `unwrap` / `expect`                                                 | `// UNWRAP:` | the check or contract that guarantees `Some`/`Ok`                               |
| `panic!` / `unreachable!` / `todo!` / `unimplemented!` / `assert*!` | `// PANIC:`  | why the branch cannot happen, or why aborting is the right response             |
| `// WORKAROUND:`                                                    | itself       | what is worked around and the condition under which it can go                   |

`// PANIC:` is the label this audit uses for panic paths; the project's `AGENTS.md` names only
`// SAFETY:`, `// UNWRAP:` and `// WORKAROUND:`. A comment that states the invariant without the
`// PANIC:` label counts as marked; a bare `// PANIC: impossible` does not count as a justification.

### Position and content

- The comment must end on the line directly above the construct, or sit on the same line.
- For a call inside a multi-line expression, the comment above the enclosing statement counts.
- For an `unsafe { ... }` block, a `// SAFETY:` as the first thing inside the block is also accepted.
- For `unsafe fn` / `unsafe impl` / `unsafe trait`, the comment goes above the item.
- A comment elsewhere in the function, or a general doc comment, does not count.

`// SAFETY: it is safe`, `// UNWRAP: safe` and `// PANIC: impossible` repeat the claim instead of
grounding it. A valid comment names the fact that makes it hold and where that fact comes from: the
`check` above, the caller's contract, the parser's invariant, the constructor that just ran, the
single-threaded phase, the platform guarantee.

Calibration from the existing tree:

```rust
// UNWRAP: the list was just checked to hold exactly one mode.
let mode = modes.pop().unwrap();                 // justified

// UNWRAP: safe.
let mode = modes.pop().unwrap();                 // vague, flag

// SAFETY: single-threaded at this point, and the call only resets `SIGPIPE` to the
// disposition a fresh process already carries; it installs no handler of our own.
unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL); }   // justified
```

### Test code

`#[cfg(test)]` items and files under `tests/` are counted and classified `test-only`.

- `unsafe` is still held to `// SAFETY:`; it is unsafe in every build.
- `unwrap`/`expect`/`assert*!` in tests are conventional; list them in a separate table with severity
  `Info`, and do not count them as production violations.
- A test-only `allow` is recorded; a missing reason is `Low`, not `High`.
- Do not ask for comments in production code because a test unwraps its result.

## Severity

- **High** — production `unsafe` with no `// SAFETY:` (or one that does not state the invariant); a
  crate-wide `allow` with no reason; `#![allow(warnings)]` or a group that hides everything; a
  justification contradicted by the code.
- **Medium** — production `unwrap`/`expect`/panic path with no or vague justification; an item-level
  `allow` with no reason; an over-broad or duplicate `allow`; `// WORKAROUND:` with no exit condition.
- **Low** — a terse but specific justification that could be clearer; a likely-unnecessary `allow`
  that cannot be proved from text; a test-only missing reason.
- **Info** — counts, test-only conventional escapes, and patterns worth keeping.

## Report shape

Both files start with a title that carries the timestamp, then the scope, then the findings. Sort
findings by severity and then by path and line. Give every finding a stable ID.

### `__QR_CLIPPY_ALLOW.md`

````markdown
# Clippy allow audit — <timestamp>

> Read-only textual audit. No build, test or Clippy run was made.

## Scope and method

- Roots: ...
- Files read: N; suppressions found: M.
- Excluded: generated files, third-party, non-Rust.
- Subagents: used (<areas>) / unavailable.
- Method: grep and read only; no `cargo`, no tests.

## Summary

| Status      | High | Medium | Low | Info | Total |
| ----------- | ---- | ------ | --- | ---- | ----- |
| missing     |      |        |     |      |       |
| vague       |      |        |     |      |       |
| over-broad  |      |        |     |      |       |
| unnecessary |      |        |     |      |       |
| duplicate   |      |        |     |      |       |
| test-only   |      |        |     |      |       |
| justified   |      |        |     |      |       |

## Findings

### High

#### CA-001 — `path:line`

- **Lints:** `clippy::...`
- **Scope:** item / module / crate / macro / test-only
- **Justification:** none / `reason = "..."` / comment at `path:line`
- **Why flagged:** ...
- **Evidence:**
  ```rust
  ...
  ```
- **Suggested action:** ... (not applied)

### Medium

...

### Low

...

## Inventory

| ID     | Location | Lints | Scope | Justification | Status  |
| ------ | -------- | ----- | ----- | ------------- | ------- |
| CA-001 | ...      | ...   | ...   | none          | missing |

## Not checked

- Whether each lint actually fires (requires Clippy).
- Compilation, tests, runtime behaviour.
- Generated files, third-party code, C# and C.

## Previous reports

- Backed up `__QR_CLIPPY_ALLOW_<YYYYMMDD>_backup_<n>.md` (or "none").
````

### `__QR_WORKAROUND.md`

````markdown
# Workaround and escape-hatch audit — <timestamp>

> Read-only textual audit. No build, test or Clippy run was made.

## Scope and method

- Roots: ...
- Files read: N.
- Excluded: generated files, third-party, non-Rust.
- Subagents: used (<areas>) / unavailable.
- Method: grep and read only; no `cargo`, no tests.

## Summary

| Construct        | Production | Test-only | Unmarked | Vague |
| ---------------- | ---------- | --------- | -------- | ----- |
| `unsafe`         |            |           |          |       |
| `unwrap`         |            |           |          |       |
| `expect`         |            |           |          |       |
| panic / assert   |            |           |          |       |
| `// WORKAROUND:` |            |           |          |       |

## Findings

### High

#### WA-001 — `path:line`

- **Construct:** ...
- **Marking:** none / `// SAFETY:` at `path:line` / ...
- **Why flagged:** ...
- **Evidence:**
  ```rust
  ...
  ```
- **Suggested action:** ... (not applied)

### Medium

...

### Low

...

## Test-only escapes

| Location | Construct | Marking | Note               |
| -------- | --------- | ------- | ------------------ |
| ...      | ...       | ...     | conventional; Info |

## Justified escapes (spot-check)

| Location | Construct | Marking | Why it holds |
| -------- | --------- | ------- | ------------ |
| ...      | ...       | ...     | ...          |

## Coverage

| Area | Files with events | `unsafe` | `unwrap` | `expect` | panic | `WORKAROUND:` |
| ---- | ----------------- | -------- | -------- | -------- | ----- | ------------- |
| ...  | ...               | ...      | ...      | ...      | ...   | ...           |

## Not checked

- Compilation, tests, runtime reachability, security impact.
- Generated files, third-party code, C# and C.

## Previous reports

- Backed up `__QR_WORKAROUND_<YYYYMMDD>_backup_<n>.md` (or "none").
````

The "Justified escapes" table is a spot-check, not every occurrence; say so, and let `Coverage` carry
the counts. The inventory in `__QR_CLIPPY_ALLOW.md` is complete because every suppression is one the
user asked to check.

## Writing the reports

1. Determine the timestamp once and use the same value in both titles.
2. For each target that exists, **move** it to a backup before writing:
   - `__QR_WORKAROUND.md` → `__QR_WORKAROUND_<YYYYMMDD>_backup_<n>.md`
   - `__QR_CLIPPY_ALLOW.md` → `__QR_CLIPPY_ALLOW_<YYYYMMDD>_backup_<n>.md`

   `<n>` starts at 1 and is the lowest positive integer not already present for that target and date.
   The request's `XX` is the date. Do not edit or delete a backup.

3. Write both reports with `write_file`, even when a report has no findings. An empty audit is a
   result: say "no findings" and give the counts.
4. If a target exists but the move fails, stop and report it; do not overwrite the old report.
5. Leave the reports in the project root. They match the repository's `__*` scratch convention and are
   ignored by git; do not force-add them.
6. Do not delete older backups.

If the user explicitly asked for only one of the two checks, write that report, say the other was not
run, and do not claim it was clean.

## Subagents

Use them when the tool is present, because the sweep is wide and read-only. The parent still owns
classification, deduplication and writing.

1. Check the tool list. `spawn_agent` (or an equivalent) present → use it. Absent → do the sweep
   yourself and write `subagents: unavailable` in both reports.
2. Split by top-level area: `app/`, `modules/`, `utils/`, `src/`, `dev/`, `tests/`. Split
   `app/cli/rola` by file group if one agent would have too much to read. Use disjoint areas and tell
   each agent its exact area.
3. Every prompt must be self-contained: the agent does not see this conversation. Include the project
   root, the area, the read-only rule, the two checks, the marking labels, and the output schema. Tell
   it to read `.agents/skills/project-quality-review/SKILL.md` for the full rules.
4. Forbid edits and runs in the prompt: no `write_file` / `edit_file` / `move_path` / `delete_path`, no
   `cargo`, no tests, no Clippy, no git writes. It returns findings only; the parent writes the reports.
5. Ask for `path:line | construct | detail | status | severity | why`, plus `coverage: N files` and
   `subagents: none (read-only)`.
6. **Verify before merging.** Open every line a subagent cites. Correct the path/line, drop
   hallucinations, and deduplicate by location and construct. A subagent's "no findings" without a
   coverage count is not accepted: re-read that area or ask again.

Prompt template:

```text
You are a read-only auditor for the rorolala Rust tree at <project root>. Do not modify anything:
no write_file, edit_file, move_path or delete_path; no cargo, clippy, tests, formatters or git
writes. Read `.agents/skills/project-quality-review/SKILL.md` for the rules, then audit only <area>.

Report every:
1. #[allow]/#![allow]/#[expect]/#![expect], including inside cfg_attr, with its justification and
   status missing/vague/justified/over-broad/unnecessary/duplicate/test-only;
2. unsafe, .unwrap(), .unwrap_err(), .expect(), .expect_err(), panic!, unreachable!, todo!,
   unimplemented!, assert*! outside tests, with the marking found (SAFETY/UNWRAP/PANIC/none) and
   whether it is adjacent;
3. // WORKAROUND: markers with their reason and exit condition.

Return one Markdown table with columns: path:line | construct | detail | status | severity | why.
Add `coverage: N files` and `subagents: none (read-only)`. Cite only lines you have opened. Do not
write any file.
```

## Noise control

Do not report:

- formatting, naming, or style that neither check covers;
- a justified entry as a finding (inventory only);
- the same construct twice because a macro body is used twice;
- every test unwrap as a production violation;
- a lint allow inside generated code (report the generator);
- "tests pass/fail" — no test was run.

## Before you finish

1. Re-open every cited finding line; correct or drop anything that moved.
2. Check that the summary counts equal the rows.
3. Re-read both reports: the title carries the timestamp, and the scope states roots, exclusions and
   subagent status.
4. Run `git --no-optional-locks status --porcelain` and confirm no source file changed. The reports are
   gitignored, so their absence from status is expected.
5. Confirm backups: for each target that existed, exactly one new backup, and its name is written in
   the new report.
6. Tell the user: counts, the highest-severity findings, the two paths, the backups, and that no
   build, test or Clippy was run.

## If asked to fix

This skill is review-only. If the user asks to fix a finding, stop the audit and say that fixing is a
separate task with its own diff; do not slip edits into the review. "Just add the comment" is an edit
too.

## When the request is unclear

Ask, with choices and their costs, before starting:

- Which roots — the whole tree or one crate?
- Both checks or one? (Default: both.)
- Report language, if the request does not settle it.

Do not guess a narrower or wider scope.
