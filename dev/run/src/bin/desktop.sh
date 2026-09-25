#!/bin/sh
# The Desktop program on its own, laid out where an export lays it out, with the plugins that ship
# with it beside it.
set -eu

. "$(dirname "$0")/../lib/common.sh"

publish_desktop "$DESKTOP_DIR"
publish_plugins
