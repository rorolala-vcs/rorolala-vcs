#!/bin/sh
# Runs clippy over the workspace; warnings are errors.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# shellcheck disable=SC2086
$CARGO clippy --workspace --all-targets --all-features -- -D warnings
