# rorolala-chunking

How content is cut into the pieces a store keeps, and the one value that says which cut a write makes.

`Chunker` is the doing: content in, the ranges it is cut into out. `Chunking` is the choice: it says
_which_ cut a write will use, and it is what a backend's algorithm choice carries. Both are here
rather than in the store because a cut is a domain of its own — see [The boundary](#the-boundary).

The general cuts are `Whole`, `Fixed`, `Cdc`, `Text` and `Zip`. They are the ones a store falls back
on: `Whole` when nothing is asked for, `Text` and `Zip` when content says it is text or a packed
container. They are deliberately not tunable, and a harness outside this crate measures strategies
against them so that the measurement does not move when a strategy does.

The strategies are beside them, one file each in `src/policies/`. Adding one is adding a file: the
crate's build script reads the `impl Policy` blocks and writes the registry they are found through,
so there is no list anybody has to edit — which is what lets several people write several formats
without ever colliding. A policy's corpus and its score live in `corpus/`, and
`dev/chunker-harness` is what writes and takes them.

## The boundary

Nothing here is remembered by a store. What a chunked entry records is the ordered list of the chunks
it was made of — the manifest — which says nothing about how the cuts were found. So content cut by
_any_ chunker reads back the same, and a chunker that was used to write yesterday's objects does not
have to exist to read them.

That is what makes the boundary worth having: the set of cuts can grow without a store changing, and
without anything already written becoming unreadable.

It is also why a chunker must be **deterministic**. The same content has to cut the same way every
time, or one file gets two manifests on two writes and the store holds the same content twice under
one key.

## Why it is a crate of its own

A per-format strategy has to implement `Chunker` without depending on the store, and the store has to
be able to reach whatever strategies exist. With the trait inside storage those two are a cycle.

The shape is the one the daemon and the protocol already have: the trait and the general cuts live
below, the strategies live beside them, and the store is above and asks. `modules/storage` re-exports
everything here, so a caller of storage never names this crate.
