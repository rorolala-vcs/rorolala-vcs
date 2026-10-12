# rola-jj

Jujutsu, as a rola program: `rola jj` is `rola-jj`, reached the way every program beside `rola` is
(see `app/cli/rola/src/external.rs`).

It is jj's own command line — the commands, their arguments and their help are `jj-cli`'s, so this
program adds none of its own — run over a store that is rola's rather than Git's. What jj would
reach Git for is left out of the build (`jj-lib` and `jj-cli` are depended on with
`default-features = false`), and a backend named `rola` is registered in its place.

That backend is a skeleton: every method of `jj_lib::backend::Backend` is a `todo!()`, so this
program compiles and its command line answers, while anything that would actually read or write the
repository stops where the work has not been done. See `src/backend.rs`.
