#!/bin/sh
# Builds the API documentation, then opens it in a browser.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# shellcheck disable=SC2086
$CARGO doc --workspace --no-deps --all-features --open
