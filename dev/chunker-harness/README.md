# rorolala-dev-chunker-harness

Scores chunking strategies against the frozen general-purpose yardsticks.

A chunking strategy is a function from content to the ranges it is cut into. What a strategy is *worth*
is how much of a version chain the store ends up holding because of it, how much one saved edit costs,
and how fast the cut is. This program answers those for every chunker it is given, over a corpus of
version chains, and prints the comparison.

```sh
cargo dev chunker                        # measure every corpus, against the yardsticks
cargo dev chunker --score                # and write each policy's SCORE.md beside its corpus
cargo dev chunker --corpus ~/my-corpus   # measure chains that live somewhere else
cargo dev chunker --generate synthetic   # write a synthetic chain set, then measure it
cargo dev chunker --check                # validate the corpora, cut nothing
cargo dev chunker --store                # also write it into a real store and compare
```

## What is frozen, and why

The yardsticks 鈥?`whole`, `fixed-4k`, `fixed-64k`, `cdc-1k`, `cdc-4k`, `cdc-16k`, `cdc-64k`, `text`
and `zip` 鈥?are the cuts the store already falls back on, unchanged and untuned. They are not
selectable from the command line and they are not policies. That is exactly what makes them the
yardsticks: their numbers do not move when a strategy is written, so a score taken this year can be
compared with one taken in three.

A strategy's score is four ratios against the best of them *on the same chain*, combined by a weighted
geometric mean. `score.toml` holds the weights, the version the numbers are taken under, and the floors
a chunker has to clear. The report prints the corpus digest beside the score for the same reason: a
score whose corpus moved is a score of something else.

## Time is not scored

It is a floor and a column of the terminal report. A weighted timing term would make a laptop and a
build machine disagree about a strategy, and a score that accumulates cannot afford that. The floors
are what keep a strategy that is beautifully small and catastrophically slow from looking like an
option.

A committed `SCORE.md` leaves the rate column out altogether, which is what lets the gate regenerate it
and compare it byte for byte: a rate is the one number here that depends on the machine it was taken
on.

## Correctness is a veto

A chunker whose ranges do not cover its input, that cuts the same input two ways, or whose ranges do
not put back together as the content, has no score at all. The three are reported by name rather than
folded into a number, because each of them is a bug and not a tradeoff.

## A corpus

```
modules/chunking/corpus/<policy>/<pattern>/[<lane>/]<version>.<ext>
```

- `<policy>` is the registered policy name the chains belong to.
- `<pattern>` is what the chain exercises: `big-file`, `in-place-edit`, `metadata-only`,
  `full-rewrite`, `shared-asset`.
- `<lane>` is a path, when a pattern is a resource shared between two paths. Without it there is one
  lane. Two lanes are measured as one chain, so what the two have in common is counted as
  deduplicated 鈥?which no single chain shows.
- `<version>` is a number starting at `000` and counting up by one. A gap is a version somebody
  deleted, and measuring around it would compare a history nobody has.

The directory is named after the **registered policy**, not after the format: a corpus is what judges
one strategy, and a directory named after a format would have to be told which of the strategies for it
it means. A `policy.toml` beside the patterns is what says the directory is a policy's 鈥?it records how
the corpus was made 鈥?and a directory without one is a chain set measured against the yardsticks alone.

## The bytes are not carried; the record is

A corpus is megabytes of a format's own files, several per pattern. A repository that carried one set
per format would be one nobody clones, so `modules/chunking/corpus/.gitignore` ignores everything in
it except the two small files that make a corpus a record rather than a directory of pictures:

- `policy.toml` 鈥?what the corpus is and how it came to be;
- `SCORE.md` 鈥?what a measurement over it came to.

The chains themselves are local, and there are two ways to have them: put the files where the
directory says they go, or measure them where they are with `--corpus <dir>`. Either way they are the
contributor's: a corpus captured from a paint program is a picture of somebody's work.

What that costs is stated in every `SCORE.md`: the digest names the bytes exactly, so two scores over
one digest can be compared, and a machine without the bytes cannot re-derive the number. A score is a
record the author stands behind, which is why it says so where the number is.

## What a corpus is, and what a score over it means

A policy's corpus is meant to be *captured*: files a real tool wrote, edited the way that tool edits
them. The plan for this work says as much, and the reason is that a score is only as good as the
edits it was taken over. `policy.toml` is where a corpus says what it is and how it was made, and a
corpus of files this repository drew itself says so there.

The synthetic chain set 鈥?`--generate synthetic` 鈥?is not a corpus for any policy. It exists so the
yardstick can be shown to work, and so the model of the store can be shown to agree with a store byte
for byte; it is generated rather than carried, and ignored by git.

## The model, and the store

`measure.rs` predicts the bytes a chain costs by reproducing what `write_content` does 鈥?one range is
one object, more than one is an object each plus a manifest, and an entry already held is not written
again. `store.rs` writes the same chain into a real store and measures its own `obj/` and `manifest/`.
`--store` compares the two, and the test beside `store.rs` asserts they are equal rather than close:
the numbers in a report are the store's, and the model behind them is shown to be the store's
arithmetic rather than an approximation of it.

## The record

`--score` writes `SCORE.md` beside each policy's corpus: the corpus digest, the patterns, the version
of the weights, whether the policy claims a container, the table, and one sentence saying whether it
beats the best general chunker and by how much. It is generated and committed rather than written by
hand, and the `generated` step of the gate regenerates it and asks git whether it moved 鈥?a number
nobody re-derives is a number that stops being true.

