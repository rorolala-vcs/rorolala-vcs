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
