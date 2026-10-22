#!/bin/sh
# Checks that the files the build writes into the tree are the ones the sources produce.
#
# Two kinds of file are generated into `src/` rather than into `OUT_DIR`, so that a change to either
# shows up in a diff beside what caused it: `modules/chunking`'s registry and its module list, written
# by that crate's build script, and each policy's `SCORE.md`, written by the harness. The price of that
# arrangement is a generated file that can be committed while it disagrees with its sources, and
# nothing in a build notices — the build would simply rewrite it. This is what notices: generate, then
# ask git whether anything moved.
#
# The corpus itself is checked too, because `--score` reads it and the numbers are only comparable
# with another score taken over the same bytes: a corpus that changed without a note is a score that
# silently means something else.
#
# What is compared is the working tree against `HEAD`, so this passes on a committed tree — which is
# what a gate runs on. Before a commit it reports the generated files as changed, which is true: they
# are not committed yet.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# shellcheck disable=SC2086
$CARGO build --quiet -p rorolala-chunking

# The score is taken in the profile the numbers were taken in, so that a difference is a difference in
# the code or the corpus rather than in how the harness was built. `SCORE.md` carries no timing, which
# is what lets it be compared byte for byte at all.
# shellcheck disable=SC2086
$CARGO run --quiet $RELEASE_FLAG -p rorolala-dev-chunker-harness -- --score

# `--exit-code` is what makes a difference a failure: `git diff` alone always succeeds.
git diff --exit-code -- \
	modules/chunking/src/registry.rs \
	modules/chunking/src/policies/mod.rs \
	modules/chunking/corpus
