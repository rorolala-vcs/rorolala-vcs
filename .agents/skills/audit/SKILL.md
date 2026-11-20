---
name: audit
description: Audit the current state of rorolala and record it under `__AUDIT_<subject>_<YYYYMMDD>/README.md` without touching code. Every run appends a dated `# Audit - <date> - <hash>` section to the same README, so one subject can be audited again while the earlier snapshots stay. Use when the user asks for an audit, a review, a check or a health assessment, or asks what the current state is, with no code change allowed.
---

# Audit

An audit records what the tree is, with evidence, and leaves the code alone. One subject gets one
directory; each run appends a dated section, so the log shows how the subject moved.

## When to use it

Load this skill when the user asks what the current state is: an audit, a review, a check, a health
assessment, or an evaluation that must stay out of the code. The trigger is the intent, not a fixed
phrase: any request shaped as "tell me what is there" belongs here.

## Shell variants

`git` and `cargo` take the same arguments in every shell. The commands that differ are given in both
forms where they appear, and the notes below cover the rest. In `pwsh` 7 and Windows PowerShell 5.1,
the exit code of a native command is `$LASTEXITCODE`; `$?` is a boolean there. Windows PowerShell 5.1
has no `&&`, so run chained lines one at a time.

## Non-negotiables

- **Do not edit source.** The only file this skill writes is its own record.
- **Do not run** `cargo ci`, a build, a formatter or a test unless the user asks for the gate reading.
  When you do, record the command and its exit status, and mark the entry as including a gate run.
- **Ground every finding.** Open the line before citing it. A claim without `path:line` or a command
  and its output is marked `unverified`.
- **Never rewrite an earlier section.** An audit is a snapshot. A later run adds a section and leaves
  the past as it was. When an earlier claim was wrong, say so in the new section and name the old one.
- **Report prose follows the user's language.** Paths, identifiers and snippets stay verbatim.

## Before the audit

1. Restate the audit in a few lines: the subject, the scope, the record path, and the read-only rule.
2. Fix the scope. When the user names a crate, a directory or a behaviour, audit that. When nothing is
   named, ask, or audit the whole program and say so.
3. Get the pair for the heading:

   ```sh
   git rev-parse --short=7 HEAD
   date +%F
   ```

   ```powershell
   git rev-parse --short=7 HEAD
   Get-Date -Format 'yyyy-MM-dd'
   ```

4. Look for `__AUDIT_<subject>_*` in the project root. When it exists, read the README and append; the
   frontmatter and the earlier sections stay untouched. Say which directory you reuse or create.
5. Gather evidence with read-only means: reading, `rg` or `Select-String`, `git --no-pager log`,
   `git --no-pager diff`, and running the program when its behaviour is the subject. `grep` is not a
   Windows command; `rg` works in both shells when it is installed, and
   `Select-String -Path <files> -Pattern '<regex>'` is the Windows PowerShell 5.1 fallback.

## Where the record lives

`__AUDIT_<subject>_<YYYYMMDD>/README.md` in the project root, the date being the day of the first
audit. Supporting files (a captured output, a table) may sit beside it. `__*` is gitignored, so the
record is local unless the user force-adds it.

## The frontmatter

```markdown
---
base-commit: 4f3a9c1
date: 2026-11-20
---
```

`base-commit` is the hash of the first audit and `date` its date. Neither changes on a later run; the
section heading carries the current pair.

## The section

Sections are separated by `---`, newest last:

```markdown
# Audit - 2026-11-20 - 4f3a9c1

## Scope

## Method

## Summary

| State  | Count |
| ------ | ----- |
| High   | 1     |
| Medium | 2     |
| Low    | 0     |

## Findings

### High

#### AD-001 `path:line`

What is wrong, what it costs, and the evidence.

## Changes since the previous entry

Empty on the first run. On later runs, what moved since the last section, with the commits between
them.

## Not checked

---

# Audit - 2026-12-02 - 8b1d0aa
```

A finding states the condition, the consequence and the evidence. `AD-<nnn>` numbering continues
across the sections of one directory; never reuse a number.

## Severity

- **High**: the tree is wrong, or about to be, in a way that costs a user data, a release or a
  rewrite. A build broken on `main`, an FFI contract that lies, a documented behaviour that does not
  exist.
- **Medium**: a defect a user meets, or a rule the project states and the tree breaks, with a
  workaround at hand.
- **Low**: drift in documentation, naming, tests or tooling that costs time later.

## Re-auditing the same subject

1. Read the last section first; the new delta starts from it.
2. Use the current hash and date in the new heading; leave the frontmatter alone.
3. State what was rechecked. A finding that no longer holds gets a line in the new section, named by
   its old id and closed with the commit that fixed it.
4. When the subject changed materially, keep the directory and say what the new subject covers.

## Finishing

1. Check every count against the rows.
2. Re-read the new section: scope, method, evidence, and the read-only statement.
3. Run `git --no-optional-locks status --porcelain` and confirm that no source file changed.
4. Tell the user the record path, the counts, the highest finding, and what was not checked. Say
   whether a gate run was part of the audit.

## If asked to fix

This skill is review-only. Fixing is a separate task and starts with the `plan` skill, so the fix has
its own diff and its own verification. When the user asks for a fix mid-audit, stop the audit and say
so.
