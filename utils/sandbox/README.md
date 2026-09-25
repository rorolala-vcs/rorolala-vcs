# rorolala-utils-sandbox

A place for a suite to run the built programs and look at what they did.

The integration suites under `tests/` are programs that run the programs this workspace
builds. Each of them needs the same few things before it can check anything: a directory
of its own to write files in, the programs themselves, and a way to start one that serves
and stop it again. That is what this crate is — the part of running a program that every
suite does, kept in one place so that a suite is only what it is checking.

A [`Sandbox`] is the directory: named after the suite, emptied when [`Guard::new`] makes it
and removed when the [`Guard`] it is made into goes out of scope, so a rerun starts clean
and a run leaves nothing behind. The programs come from [`bin_dir`], and [`command`],
[`run`] and [`serve`] turn one of them into a command, a finished run whose output can be
read, or a process that is still serving — which [`Serving`] is the guard for.
