#!/bin/sh
# Builds the C ABI artifact — a release cdylib and staticlib — and, as a side effect of the root
# build script, the C header that describes it.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# `CARGO` is deliberately not quoted, here and in every other script: it may be several words, as in
# `CARGO="cargo --offline" ./run.sh export`.
# shellcheck disable=SC2086
$CARGO build $RELEASE_FLAG --all-features -p rorolala-ffi
