# rorolala-tree

A directory versioned as one file: the tree structure a version names, and what a tree means to the
rest of Rola.

The structure is the whole of a tree version's content: the paths it holds, what each of them is,
and the stored content each file stands for. It is a flat, canonically ordered table written as one
content object, so the same files always make the same key. Nothing here reads a Layout or a store
yet: the object model is the foundation the rest of the tree work is built on.

See [`__PLAN_TREE_SYSTEM/`](../../__PLAN_TREE_SYSTEM/) for the design this crate implements.
