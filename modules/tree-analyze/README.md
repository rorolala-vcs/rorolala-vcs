# rorolala-tree-analyze

What a Workspace's tree looks like beside the Layout it works from: what a Layout names that the
tree no longer holds, what the tree holds that no Layout names, what moved, and what changed.

It only reads. What it finds is [`TreeDiff`], and what it remembers between readings is a cache
under the Workspace's data directory, so a file that has not changed is not read again.

See `tree_diff` for the analysis and `cache` for what is kept.
