---
name: cli-help-check
description: Check the layout of rola's `--help` copy after it is written or changed — right-column alignment, the four-space gap after the longest left item, real Unicode width, and overlong lines. Runs heuristic Python checkers that report candidates rather than asserting, because real help blocks may hold several tables, prose and sections without a right column. Use when the user asks to check rola CLI help text or has just edited a `help` key under `app/cli/rola/i18n/`. Do not use for error hints (`err_*_help`), the welcome text, or the desktop program.
---

# Checking rola's `--help` copy

The `--help` branch is every `#[help(buffer)]` handler. It prints `t!("<key>.help")` through
`trd!`, and the text is **hand-aligned in `app/cli/rola/i18n/*.yml`** — there is no layout engine.
This skill checks four things and leaves the verdict to a person: a help block can contain several
tables, prose, and sections with no right column, so a single rule does not decide the layout.

## Scope

- **Only `--help`.** Every `#[help(buffer)]` handler:
  - main help: `app/cli/rola/src/main.rs`, `help_rola`, key `rola.help` in
    `app/cli/rola/i18n/rola.yml`;
  - subcommands: `app/cli/rola/src/cmd_*.rs`, keys `X.help`, plus `fs_ops.cp_help`,
    `fs_ops.mv_help`, `fs_ops.rm_help`, and the like.
- **Exclude** `err_*_help` (error hints), `welcome`, `err_unknown_command*`, and anything drawn by
  `help_line!` or `err_line!`: those are not the `--help` branch.
- `-h` reaches the same handler. Do not audit it as a separate output.
- Do not run `cargo ci`, do not build, do not reformat. A built binary is only a way to see the real
  output; checking is textual.

## What the renderer does

- `trd!` is `TextRendering::render`. It removes **paired** `**`, `__`, `~~` and `*` markers; keeps
  backticks and `<...>` as written; trims line ends; and **does not wrap**.
- Width is `unicode_width` (`utils/cli-theme/src/rendering/ansi.rs`, `display_width`): East Asian
  Wide/Fullwidth = 2, combining = 0. Backticks and angle brackets count 1 each.
- Output goes to stderr; the help exit code is `EC_HELP`.
- Consequence: the YAML's spaces are the output's spaces, except that `**` markers disappear.
  Measure the real output, or account for the markers.

## The four checks

For each two-column table in each help block and language:

1. **Right column aligned** — every right-hand text starts at the same display column.
2. **Four-space anchor** — that column is the display width of the widest left cell (including its
   leading indent) plus 4. "Widest" is columns, not characters.
3. **Real Unicode width** — CJK is 2; `，`, `：`, `（` are 2; `<`, `>` and backticks are 1. A table
   that looks aligned in an editor can be off by one or more columns.
4. **Overlong lines** — the renderer never wraps. Report lines wider than 80 columns, or the width
   the user names. A long line is a candidate, not automatically a bug.

Rules of interpretation:

- A block may hold several tables (`**Flags**` then `**Commands**`, for example). Apply checks 1–4
  **per table**. Whether the anchor is per table or per whole block is a product decision; state
  which one was used. Do not silently merge tables.
- Sections with no right column (`**Usage**`, the global-flags block, prose) are skipped.
- The `+4` rule is a project convention, not something the renderer enforces. If the copy
  intentionally uses another gap, say so rather than "fixing" it.
- A single-row table has a gap but no alignment evidence. Report it separately, with low confidence.

## Reusable checkers

Two Python 3 scripts, standard library only, in `scripts/`. They **find candidates; they do not
assert.** Both resolve the project root by walking up to `app/cli/rola/i18n`, so they can be run
from anywhere.

- `scripts/audit_help.py` — reads the i18n YAML and every `#[help(buffer)]` key; prints a per-block
  table and a compact total. It approximates `trd!` (paired emphasis markers only), so it is for
  discovery, not for the final word.
- `scripts/check_output.py` — runs the built binary and measures the real output, or checks
  captured files; `--no-color` is used. This is the authoritative width check.
  - no arguments — top-level help in `en` and `zh-CN`;
  - `checkin storage` — those commands in both languages;
  - `--command "storage ls-remote-manifests"` — a command path containing spaces;
  - `--file out.txt` — a captured output (repeatable);
  - `--strict` — exit 1 on findings (never use this as a release gate);
  - `--bin PATH` — another binary.

Run from the project root:

```sh
python .agents/skills/cli-help-check/scripts/audit_help.py
python .agents/skills/cli-help-check/scripts/check_output.py
python .agents/skills/cli-help-check/scripts/check_output.py checkin
```

If `build/bin/rola.exe` (or `build/bin/rola`) is missing or older than the edited YAML, say so. If
the user agrees, build the CLI first; otherwise check the YAML only and label the report static.

### Known limits

- A line with 2+ spaces between non-space runs is treated as a two-column row. A prose line with
  such a gap can be misread; verify it.
- The scripts use one anchor per block, not per sub-table. Re-run or reason per table when a block
  has several.
- `[[...]]` directives, links and unusual emphasis are not simulated. If a help text uses them,
  measure the real output.
- East Asian Ambiguous characters are counted as 1, matching `unicode_width`'s default.
- The `+4` gap is measured from the widest left cell's display end to the right text's first
  column. Four blank columns means `right_start = left_end + 4`.

## Workflow

1. **Restate** the scope in one or two lines: main help, all `--help` blocks, or the command the
   user named; the four checks; read-only unless the user asks for a fix.
2. **Ask about a subagent first.** This step is a good candidate for a read-only subagent, but it
   costs extra budget. Ask: “这一步可以交给子 agent 做，会消耗额外预算；要不要用？” Do it only if
   the user agrees and `spawn_agent` is available. If not, do it directly and do not claim a
   subagent was used.
3. **Discover** with `audit_help.py`; note the keys and blocks that have two-column rows.
4. **Confirm** with the real output via `check_output.py` for the main help and any suspicious
   block. Do not trust the YAML parse alone.
5. **Verify** each candidate by opening the YAML line and comparing the actual output. Discard false
   positives: multi-table blocks, prose with gaps, and intentional conventions.
6. **Report** per block and language: the widest left cell and its end column, the expected right
   column (end + 4), the actual right column, the rows that break alignment, and the widest line
   plus whether it exceeds 80. Reference files with line numbers.
7. **Do not edit** the copy unless asked. If asked, change the YAML spaces, re-run the checkers, and
   show the before/after columns. Do not change wording to fix alignment unless the user approves.

## Subagent

Ask first. If allowed:

- A subagent is read-only and does raw scanning. It runs the scripts, opens the cited YAML lines,
  and returns findings; it does not edit.
- Give it the project root, the scope, the four checks, the renderer facts, and the output columns:
  `key | lang | left cell | left end | expected right | actual right | issue | line`.
- Tell it to read this SKILL.md.
- Verify every cited line and column before merging; drop hallucinations and deduplicate.
- If the user says no, do the work yourself and write `subagent: not used` in the report.

## Report shape

```markdown
## Scope and method

- `--help` blocks checked: ...
- Real output from `build/bin/rola.exe ...`, or static YAML only
- Width: Unicode EAW (W/F = 2), matching `unicode_width`

## `rola --help`

| lang | right start | widest left | gap | expected | verdict |
| ---- | ----------- | ----------- | --- | -------- | ------- |

## Subcommands

### Alignment

### Four-space anchor

### Overlong lines

## Not checked / caveats
```

Keep the report in the user's language; keep paths, keys and column numbers verbatim.

## Non-negotiables

- Only `--help`. Not `-h` as a separate case, not `err_*_help`, not the welcome or unknown-command
  text.
- The scripts are heuristics. A listed deviation is a candidate; the person decides. Never call the
  check a gate or a test.
- Never invent a column, width or line. Measure it or open it.
- Never edit help copy, run `cargo ci`, or rebuild without being asked.
