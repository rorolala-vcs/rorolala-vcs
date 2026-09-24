#!/bin/sh
# Verifies that every crate is formatted, without rewriting anything.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# shellcheck disable=SC2086
$CARGO fmt --all -- --check
