---
name: changelog-entry
description: Record a change to rola's shipped content in `CHANGELOG.md` with `cargo changelog`. Use when a change under `app/`, `modules/`, `utils/` or the root `src/` is made, or when the user asks for a changelog entry, a release note, or how to fill `__CHANGELOG_DRAFT.md`. Do not use for `dev/` tooling, docs-only, or test-only changes.
---

# Writing a Changelog Entry

Rorolala keeps one changelog, `CHANGELOG.md`, at the repository root. It is edited only through
`cargo changelog`, which merges the draft, reorders the entries and seals a version; hand-editing
the file is how two people produce two orders. The command reference is
`dev/changelog/README.md`; this skill is the part an agent has to decide: whether an entry is
needed, and what its scope is.

## When an entry is required

A change that touches what rola actually is — its behavior, command line, desktop program, file
formats, protocol, storage, performance, or compatibility — is recorded in the changelog. That
means a change under:

- `app/` — the command line programs, the C ABI, the desktop and the launcher;
- `modules/` — the feature modules and the C# bindings;
- `utils/` — the shared facilities;
- root `src/` — the `librorolala` crate that ties the modules together.

A `dev/` change (build, CI, harnesses, this changelog editor itself), a documentation change, a
comment-only or formatting-only change, or a test-only change does not need an entry: nothing a
user of rola receives changed. A test that accompanies a content change does not get an entry of
its own; the content change does.

`refactor` is a category, so an internal reshuffle of in-scope code is recorded even when a user
would not notice it. Use `bugfix` when something was wrong, `optimized` when it works better,
`feature` when it is new, and `refactor` when the shape changed without changing what it is. If a
change sits on the boundary, ask the user rather than guessing.

## Scope: the directory, broadest first

The `scope` is the path to the changed unit, primary scope first. It follows the directory tree,
not a free-form tag or a file extension.

| change                             | scope                                 |
| ---------------------------------- | ------------------------------------- |
| `app/cli/rola`, a command          | `["app", "rola", "checkin"]`          |
| `app/desktop/Core`, the theme      | `["app", "RorolalaDesktop", "theme"]` |
| `modules/vault`, a file or area    | `["modules", "vault", "config"]`      |
| `utils/cli-theme`, colour handling | `["utils", "cli-theme", "colorize"]`  |
| root `src/ownership.rs`            | `["librorolala", "ownership"]`        |

The first element is the layer (`app`, `modules`, `utils`) or the root crate (`librorolala`). The
second is the program, crate or assembly as it is named — `rola`, `RorolalaDesktop`, `vault`, not
the directory spelling where the two differ. The rest are the areas within it, in directory order.
A plugin under `app/desktop/Plugins` may go one level deeper:
`["app", "RorolalaDesktop", "plugins", "terminal"]`.

One entry carries one hierarchy. A change that spans several areas takes their deepest common
scope, with the rest said in the body; do not put two unrelated paths in one `scope` list. If the
common scope would be so broad that it says nothing (`["app"]`), split the change into several
entries.

The order matters: within a category, `write` and `archive` put breaking entries first, then sort
by this hierarchy level by level, then by summary. Choosing the scope is choosing where the entry
will be read.

## Writing the entry

The commands write the tree, so it has to be clean unless `--allow-dirty` is given. The normal
order is:

1. Commit the content change.
2. `cargo changelog write-reset` — create or reset `__CHANGELOG_DRAFT.md`.
3. Fill the draft.
4. `cargo changelog write` — merge it into the current version's `(Unreleased)` section.
5. Review `CHANGELOG.md` and commit it.

If the content change is not committed yet and the user wants the entry now, the run needs
`--allow-dirty`; ask before using it. If `CHANGELOG.md` itself is new and still untracked, commit
it first: an untracked changelog makes the tree dirty and `write` refuses it. The draft, backup and
rewrite files start with `__` and are ignored by git, so they do not dirty the tree.

`write` consumes the draft. For a second entry, run `write-reset` again: one draft is one entry,
not one commit.

### The draft

```markdown
---
summary: Fix the blocking read in checkin
category: bugfix
scope: ["app", "rola", "checkin"]
breaking: false
---

The `checkin` command no longer blocks when the Vault answers before the reader is ready. The
stream is drained on a worker instead of the caller's thread.
```

- `summary` is one line, no trailing period, no markdown link; it becomes the entry's title after
  the scope.
- `category` is `bugfix`, `optimized`, `feature`, or `refactor`.
- `scope` is the hierarchy above. It may be a list or one string; the list is preferred because it
  states the levels.
- `breaking` marks an incompatible change. It is the only signal: the category says what kind of
  change it is, not whether a caller breaks.
- The body says what changed and why. For a breaking change it also says what a caller or a
  repository has to do; that is the part readers search for.

The changelog is written in English, like the rest of the tree's documentation.

### Categories

| category    | what it says                                  |
| ----------- | --------------------------------------------- |
| `bugfix`    | something was wrong and is now right          |
| `optimized` | it works better: faster, smaller, less memory |
| `feature`   | something new is available                    |
| `refactor`  | the shape changed; what it is did not         |

`breaking` is not a category. Choose the category that says what kind of change it is, and set
`breaking: true` separately when the change is incompatible with what callers had.

A change that does several of these is several entries.

## What not to do

- Do not run `cargo changelog archive` for an ordinary change. It dates and seals the newest
  `(Unreleased)` section, usually just before a release, and a sealed version cannot be written to
  again.
- Do not run `cargo changelog set-version` to record a change. It moves the workspace version and
  refreshes `Cargo.lock`; it is a release step.
- Do not hand-edit `CHANGELOG.md` to change the order or the numbers. `write` and `archive` reorder
  entries and renumber them from 1.
- Do not put a `dev/` or docs-only change in the changelog. If the user explicitly asks for one,
  confirm that the change really is in scope first; the rule is the shipped content, not the
  tooling that builds it.
