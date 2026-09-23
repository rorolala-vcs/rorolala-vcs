#!/bin/sh
# Formats every crate in the workspace in place.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# shellcheck disable=SC2086
$CARGO fmt --all
