# rorolala-utils-icons

The tree's icons, drawn from the descriptions beside them.

There is no code here: the crate is its build script. What it makes — `res/icons/*.ico` — is what a
program's own build script links in on Windows, and cargo does not order one build script against
another. A build dependency is the one ordering it does promise: the build script of a crate that
something depends on runs before the build script of the something. So `librorolala`, `rola` and
`rola-daemon` all depend on this crate, and a tree that was just checked out has its icons before
anything looks for one.

See `build.rs`.
