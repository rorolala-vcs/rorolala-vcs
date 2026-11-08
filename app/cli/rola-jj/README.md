# rola-jj

## Introduction

`rola-jj` runs Jujutsu's own command line over a store that is Rorolala's rather than Git's, reached as `rola jj` like every program beside `rola`. The commands, arguments and help are `jj-cli`'s whole.

## Boundary

It owns the entry point, the name and version it reports, the eight mebibytes of stack it gives the command line, and the `rola` backend it registers with jj. It does not own the command line's words or behaviour, which are `jj-cli`'s, nor the backend's work: `src/backend.rs` is a skeleton whose `jj_lib::backend::Backend` methods are `todo!()`, so anything that would read or write the repository stops there. Git support is left out of the build with `default-features = false`.
