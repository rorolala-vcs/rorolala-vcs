# rorolala-inverse-index

The reverse dependency index over a Rorolala VCS index.

The version control index is content addressed: an object is found by the hash it is named by, and
what it points at is written inside it. So a question that walks *the other way* — which variants
depend on this stored content, which versions pin this variant, which variants are based on this
version, what was made by this creator — can only be answered by reading every object.

This crate keeps that answer: for each object, the objects that point at it, with the role by which
they do. It is **derived data**, never the truth — every fact in it comes out of the objects, so it
can always be built again from them, and is only used while it says exactly what the objects say.
A version's number is the one derived *value* kept here, and it is kept only because it is
**monotone**: a number is unknown until its chain reaches the root and fixed forever after, so a
record never has to be corrected, only filled.

## Layout

Beside the index's objects, under `<index>/inverse/inverse_0.dat`, written whole and moved into
place so a write cut short leaves the file before it:

```text
magic        b"ROLAINV1"          8
format       u32, big endian
digest       BLAKE3 of the body   32
body:
  covered    the object keys the index held, in key order
  entries    per key: the dependents, and a version's number
```

The covered keys are what makes it checkable without trusting a time: a reader lists the index's
keys — which the pack index and the loose tree name without an object being read — and uses the file
only when the two agree. Anything else — absent, a format this build does not know, a digest that
does not come back, keys that have moved on — means the file is left alone and the question is
answered by reading the objects.

## Building

A rebuild reads what the records do not describe and writes them again, so a rebuild after a handful
of new objects reads those objects and nothing else: the keys the records cover are compared with the
keys the index lists, and the difference is read. What cannot be added to — the file is not there,
does not read, or covers an object the index no longer holds — is built afresh from every object. The
report says which of the three happened: `fresh`, `incremental` or `full`.

Numbers fill the same way in both: a version is its base version's one past, so the rebuild takes
every numbered version it has and walks outwards, filling each version whose base is numbered and
leaving one whose chain is cut off for a later rebuild to fill when the link arrives.

## Reading the other way

`InverseIndex::snapshot` inverts the records once into what each variant is made of and what each
version fixes, with the questions a walk asks kept beside them — which versions fix a variant, which
variants are based on a version, which merge one in, which were made of one content. That is the
shape a graph is walked in.

`Snapshot::graph` cuts a window out of it: a focus, how far back towards the root and forward away
from it to walk, whether to follow merges and one-hop branches, and a cap. What it answers with is
the variants, the versions, the edges between them, the keys the index does not hold, and a
`frontier` on each node saying which way the window stopped short — so a reader that reaches the
edge asks `Snapshot::expand` for the part beyond it rather than for the whole index again. Nothing
here reads or fetches an object: a key the index does not hold is named as missing.
