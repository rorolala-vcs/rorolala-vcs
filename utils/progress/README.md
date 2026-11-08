# rorolala-utils-progress

## Introduction

What a long run says about itself while it runs, as signals handed down to whatever is listening.

## Boundary

It owns the `Progress` handle, the tasks it begins, and the signal vocabulary of begin, advance and finish. It does not draw anything: a reader decides whether a signal becomes a bar, a record or nothing, which for the command line is `app/cli/rola/src/progress.rs` and for the desktop is its own.
