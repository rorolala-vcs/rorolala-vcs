---
name: create-chunking-policy
description: Add a format-aware chunking strategy to rorolala in four hands. Ask the user for the name and the containers it serves, scaffold an empty policy and the corpus with a `__TODO.md` in the user's own language, validate the files they supply, and only then implement, measure and iterate until the score earns a container claim or leaves it name-only. Use when the user asks to write a chunking policy for a binary format (PSD, Maya, PPT, ...).
---

# Creating a Chunking Policy

A chunking policy is a per-format strategy that cuts one file into the chunks a store keeps. It is a
`Chunker` (content in, `Vec<Range<usize>>` out) plus a registered name and the container extensions it
claims, all in one file under `modules/chunking/src/policies/`. The crate's build script reads the
`impl Policy` blocks and generates the registry, so adding a policy is adding a file; the harness under
`dev/chunker-harness` measures it against nine frozen general-purpose yardsticks.

This is an interactive, two-handed workflow in phases, and the order is part of it: the agent
asks, scaffolds and writes code; the user supplies the corpus — real files a real tool produced — and
decides whether the policy claims a container. **The splitter is written only after the corpus exists
and has been checked**, because a splitter written against imagined bytes is a guess, and a corpus the
harness refuses measures nothing. Do not invent a name, flag, extension, or file the user did not ask
for.

## Phase 1 — Ask, then restate

Before touching the tree, settle all four with the user and restate them back with their tradeoffs:

1. **The registered name.** kebab-case, e.g. `maya-binary`. It is the name the command line uses and
   the corpus directory name, and it must be unique across the build.
2. **The containers it is meant to serve.** lowercase, no dot, e.g. `mb`. Unique across the build.
   This is what `policy.toml`'s `format` records, and the claim the score may earn later. The policy
   starts claiming nothing (`EXTENSIONS = &[]`) regardless.
3. **The provenance.** Which tool and version will write the files, and what edit each version of a
   chain is. This goes into `policy.toml`'s `made-with`, which is a record for humans.
4. **The language of `__TODO.md`.** Write it in the language the user has been writing in; ask only
   when that is not clear.

If any answer is missing, ask again with the choices and what each costs. Do not start Phase 2 on a
guess.

## Phase 2 — Scaffold the empty policy

Create `modules/chunking/src/policies/<file>.rs` — the type and the registration, and **no parser**.
`<file>` and the type must not repeat each other (`maya.rs` holds `Maya`, not `MayaBinary`), or
`clippy::module_name_repetitions` fails the build.

```rust
//! A strategy that cuts a Maya binary document at ... (rewritten in Phase 5).

use std::ops::Range;
use crate::{Chunker, Policy, Whole};

/// A strategy that cuts ... (rewritten in Phase 5).
#[derive(Debug, Clone, Copy, Default)]
pub struct Maya;

impl Chunker for Maya {
    fn split(&self, content: &[u8]) -> Vec<Range<usize>> {
        // The cut is written once the corpus says what it has to beat. Until then the whole file is
        // one chunk, which is always a correct answer.
        Whole.split(content)
    }
}

impl Policy for Maya {
    const NAME: &'static str = "maya-binary";
    const EXTENSIONS: &'static [&'static str] = &[];   // no claim until the score earns one
}
```

The placeholder is not dead weight: the harness refuses a corpus whose `policy.toml` names a policy
this build does not carry, so the name has to be registered before its corpus can be read at all.

Then generate the registry:

```sh
cargo build -p rorolala-chunking
```

`build.rs` rewrites `src/policies/mod.rs` and `src/registry.rs`; neither is edited by hand. The build
stops rather than let two policies disagree, naming what is wrong:

- `` `A` and `B` both claim the name `x` ``
- `` `A` and `B` both claim `mb`, and one container is cut one way ``
- `` `X`: `x` is not kebab-case… `` — and the same shape for a non-kebab name, a non-container
  extension, a missing `NAME`/`EXTENSIONS`, or a const that is not the expected literal or array.

## Phase 3 — Scaffold the corpus and leave the user a `__TODO.md`

Create the policy's corpus directory and the pattern directories the user will fill:

```
modules/chunking/corpus/<name>/policy.toml
modules/chunking/corpus/<name>/big-file/
modules/chunking/corpus/<name>/in-place-edit/
modules/chunking/corpus/<name>/metadata-only/
modules/chunking/corpus/<name>/full-rewrite/
modules/chunking/corpus/<name>/shared-asset/
modules/chunking/corpus/<name>/__TODO.md
```

`shared-asset` may also get its lane directories now, one per path, or the user may add them with the
files; a lane is `<lane>/000.<ext>`, `001.<ext>`, ….

Write `policy.toml` beside them:

```toml
name = "maya-binary"
format = "mb"
made-with = "Maya 2024, hand-edited one object and saved"
patterns = ["big-file", "in-place-edit", "metadata-only", "full-rewrite", "shared-asset"]
```

**The empty pattern directories make the corpus unmeasurable until they are filled**:
`Corpus::read` refuses a pattern with no version (`names no version`), and `./run.sh chunker` exits
non-zero. That is the intended signal that the corpus is not ready — but it means this intermediate
state is local only. Do not run `./run.sh check` against it and do not commit it: the gate's
`generated` step runs the harness, and a `policy.toml` committed without its corpus and without a real
`SCORE.md` is a record of nothing. The empty directories are not carried by git anyway; the tree is
whole again once the versions are in place.

### What `__TODO.md` says

Write it in the user's language, as instructions to a person who knows their tool but not this
harness. It must state:

- **What each pattern is for**, one line each:
  - `big-file` — one large first version, no history: what one version of a real document costs.
  - `in-place-edit` — an equal-length rewrite in the middle: what a save of a large document looks
    like, and the case a content-defined cut exists for.
  - `metadata-only` — the body kept while the header or metadata moved: the case a container-aware
    policy exists to win.
  - `full-rewrite` — the same bytes written again: whether a cut hits when nothing changed.
  - `shared-asset` — one body shared by two paths, each edited elsewhere: cross-file deduplication,
    which no single chain shows.
- **The file names**: `000.<ext>`, `001.<ext>`, … from zero, counting by one, **no gaps**.
- **One lane, one container**: every version in one directory (or one lane directory) has the same
  extension; mixing them is refused.
- **`shared-asset` is the only pattern with lane directories** — one per path, at least two, and the
  first version of each is the same body.
- **Real files only**, written by the stated tool and version; every version is a real save, and the
  TODO should say what each save changed.
- **Nothing else in a pattern directory**: every file in one is read as a version, so a note, a
  `.DS_Store` or a `README` there is a refused corpus.
- **Real sizes**: a chain worth measuring is made of documents at the sizes the tool really writes.
- **The version files never enter git** (`corpus/.gitignore` carries only `policy.toml` and
  `SCORE.md`); files that must stay elsewhere are measured with `./run.sh chunker --corpus <dir>`.
- **Tell the agent when it is done.**

`__TODO.md` is already ignored by `corpus/.gitignore`, and it is scratch: delete it in Phase 4 once
the corpus passes.

## Phase 4 — Validate what the user supplied

When the user says the corpus is ready, check it before writing a line of the splitter.

```sh
./run.sh chunker --check      # structure only: numbering, one container per lane, non-empty patterns
```

The harness checks the shape and nothing else. Check the rest yourself, and report each finding as the
exact path and what is wrong:

- Every pattern directory has at least one version; the numbering starts at `000` and has no gaps.
- One container per lane, and it is the container the user asked for — the harness only checks that the
  lane is consistent with itself.
- `shared-asset` has at least two lanes, and their first versions are the same body.
- The bytes really are the format they claim, so far as a signature can say (`8BPS` for PSD, for
  example). The harness does not look inside a file.
- Nothing else sits in a pattern directory, and the files are real document sizes rather than a
  placeholder a tool wrote in one save.

Do not edit, rename, or replace the user's files. Name what to fix and wait. When everything passes,
delete `__TODO.md`, state in one or two lines what was checked, and go on. When something fails, say
which file and why, and stop there — a splitter written over a corpus that is about to change is work
thrown away.

## Phase 5 — Implement the policy

Replace the placeholder `split` with the format's parser. The type, the name and the empty `EXTENSIONS`
stay.

```rust
impl Chunker for Maya {
    fn split(&self, content: &[u8]) -> Vec<Range<usize>> {
        walk(content).unwrap_or_else(|| Whole.split(content))
    }
}
```

`walk` is the format's own parser; a parse it cannot complete falls back to `Whole`.

Three rules hold for every splitter:

- The returned ranges are **ordered, non-overlapping, and cover all of `content`**; concatenating them
  gives `content` back byte-for-byte. Content that cannot be understood is returned as one chunk
  (`Whole`), never refused.
- It is **deterministic**: the same bytes cut the same way, every time.
- It does **not decompress or re-encode**: the pieces are the original bytes, so reassembly is exact.

What a format-aware cut usually does is find the spans that are the container's **contents** — a
member's payload, a layer's channel bytes, a tagged block's data — and cut those out whole, letting
headers, lengths and what belongs to no span be ranges of their own. That is what makes an edit to one
part leave every part it did not touch a chunk the store already has. A section that cannot be taken
apart is kept whole rather than half-cut; the fallback is what keeps that safe.

Put the parser's own tests in the file. A synthetic document built by the test is what checks coverage,
reassembly, determinism and the fallbacks; a pure helper gets a `#[cfg(test)]` module in its own file,
the same as anywhere else in the crate.

## Phase 6 — Measure, then iterate

```sh
./run.sh chunker          # the crate's own corpus/, by default
./run.sh chunker --store  # also write through a real store and compare it to the model
```

The report prints a per-pattern table and then an aggregate table. The policy appears as a row because
`policy.toml` exists; a name this build does not carry is an error that lists the names it does carry.
The row is ranked against the nine frozen baselines — `whole`, `fixed-4k`, `fixed-64k`, `cdc-1k`,
`cdc-4k`, `cdc-16k`, `cdc-64k`, `text`, `zip`.

Read the summary against the best baseline, then iterate: adjust the splitter, re-run, and report each
round's score and the gap to the user. This is the optimization loop — keep going until the number
stops moving or the user says stop. `--store` is the stricter check that the model's bytes equal the
store's real bytes.

## Phase 7 — Claim the extension, or don't

- **Wins or ties the best baseline** → set `EXTENSIONS` to `&["mb"]`, then rebuild with the command
  from Phase 2 so the registry's `claims` column changes. From then on an `.mb` write is cut by this
  policy.
- **Loses** → keep `&[]`. It stays usable by name only, and never overrides the fallback.

At write time the cut is chosen in this order: the `--policy` the run named → the file's recorded
preference → an extension claim → a ZIP signature → text → whole.

## Phase 8 — Write the record and commit

```sh
./run.sh chunker --score
```

This generates `SCORE.md` beside the corpus: the corpus digest, the patterns, the weights version,
whether the policy claims a container, the full table, and one sentence stating the win/loss. **It is
generated — do not hand-edit it.**

Commit exactly these, and nothing else from `corpus/`:

- `modules/chunking/src/policies/<file>.rs`
- `modules/chunking/src/policies/mod.rs` and `modules/chunking/src/registry.rs` (generated, but
  committed)
- `modules/chunking/corpus/<name>/policy.toml` and `modules/chunking/corpus/<name>/SCORE.md`

The scaffold from Phases 2–3 is committed only as this finished set: a `policy.toml` without a real
`SCORE.md` records nothing, and the placeholder splitter is not the policy. The version files stay out:
`corpus/.gitignore` carries only `policy.toml` and `SCORE.md`, which is what keeps the repository from
bloating as formats multiply.

Then:

```sh
./run.sh check
```

The `generated` step regenerates the registry and `SCORE.md` and compares the working tree against
`HEAD`, so it is green only after the commit — by design, not a failure.

## Phase 9 — Using it (reference)

```sh
rola track big.mb --policy maya-binary      # this write, and record it as the file's preference
rola track big.mb -m "edited one object"    # later writes reuse the recorded preference
```

- `--policy` applies to every file in that run; an unknown name is refused before any file is read, with
  the available names listed.
- The preference is recorded on the version, and survives `sync`, `fetch`, `checkin` and
  `layout entry`.
- A file whose recorded preference names a policy this build lacks is an error and writes nothing —
  there is no silent fallback, because falling back writes a version that deduplicates against nothing
  already stored.
- A policy with `EXTENSIONS = &[]` is reachable only by being named.
- Reads are unaffected: the cut matters only at write time; reading relies on the manifest alone.

## Known limits

1. **The preference is local.** It survives sync/fetch for the author, but a teammate who checks out the
   same files has no preference and gets the default cut. Team-wide sharing needs the Vault's Layout to
   carry it, which touches three remote-write inputs and the C ABI.
2. **The harness does not check that the corpus extension is claimed.** It validates structure (version
   numbering, one container per lane, non-empty patterns) but not that a claimed policy matches the
   bytes; a misplaced container is not refused. Phase 4 is where that is caught.
3. **`SCORE.md` has no MB/s.** The rate depends on the machine, and the `generated` step compares
   byte-for-byte, so speed lives only in the terminal report.
