# rorolala-layout

The layout a Workspace and its Vaults share: what each keeps where, and where one finds the other.

It is the one thing about a version control that changes — everything else is content-addressed and
never does — so it is kept as a log of changes appended to, replayed into memory, and answered from
there. See the crate's `layout` module for how it is kept and `path` for what it calls a path.
