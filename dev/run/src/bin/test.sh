#!/bin/sh
# Runs the test suites. Deliberately left on cargo's default profile: release builds turn
# `debug_assert!` off, which is the opposite of what a test run wants.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# shellcheck disable=SC2086
$CARGO test --workspace --all-features
