#!/bin/sh
# The full gate, and nothing besides: the gate already builds everything that ships, so asking for a
# build after it would be asking twice.
set -eu

. "$(dirname "$0")/../lib/common.sh"

again check
