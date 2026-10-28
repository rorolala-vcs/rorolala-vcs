# rorolala-ignore

The ignore rule set a Workspace or a Vault is managed by.

The syntax is gitignore's, but the rules are not scattered through the tree the way a
`.gitignore` is: one directory — `info/ignore/` — holds them all, a file per rule set, read
recursively at whatever depth they sit at. A Vault's own directory of them is what a fetch
brings down; a Workspace keeps what it fetched under `vaults/<name>/` inside its own, so a rule
of the Workspace's always sits shallower than one that arrived.

Shallower decides over deeper, and a name that sorts later decides over one that sorts sooner —
which is gitignore's "the last matching pattern decides" read across files rather than within
one. That is what lets a Workspace suppress a Vault's rule: the Vault's `*.mb` is applied before
the Workspace's own `!*.mb`, and the later one is the one that stands.
