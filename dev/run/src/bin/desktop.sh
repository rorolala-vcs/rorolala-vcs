#!/bin/sh
# The Desktop program on its own, laid out where an export lays it out, with the plugins that ship
# with it beside it.
#
# The C ABI is built first, and deliberately: a plugin that answers who holds an entry binds the
# library, and both the library and the header the bindings are generated from are the Rust side's
# output. Building the Desktop without them would be building against the last ones there happened to
# be, or failing at the header.
set -eu

. "$(dirname "$0")/../lib/common.sh"

again lib

# The whole solution is restored and built once for the two layouts below rather than a project at a
# time: the restore is shared, and one MSBuild over the solution is what builds the host and the
# plugins together.
build_solution

publish_desktop "$DESKTOP_DIR"
publish_launcher "$DESKTOP_DIR"
publish_plugins
