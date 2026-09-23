#!/bin/sh
# Everything that ships: the library and the programs.
set -eu

. "$(dirname "$0")/../lib/common.sh"

again lib
again bin
