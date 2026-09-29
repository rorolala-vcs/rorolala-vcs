# rorolala-layout

The layout a Workspace and its Vaults share: what each keeps where, and where one finds the other.

It is the one thing about a version control that changes — everything else is content-addressed and
never does — so it is kept as a log of changes appended to, replayed into memory, and answered from
there. See the crate's `layout` module for how it is kept and `path` for what it calls a path.

A layout may also be written down as a **`.rolayout` file**: a lossy snapshot of one moment of it —
what each path named, the `Uuid` the file is known by upstream, and the version it was at — so the
work can be put back elsewhere. Written with the index and the store beside it, the file is *packed*
and also names every key a checkout needs. See the crate's `file` module for the format.
