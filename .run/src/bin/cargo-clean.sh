#!/bin/sh
# Removes cargo's build output. `$BUILD_DIR` survives: it holds an export, not a build.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# shellcheck disable=SC2086
$CARGO clean
