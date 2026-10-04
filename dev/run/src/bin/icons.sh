#!/bin/sh
# Draws the Desktop's own pictures from the pinned Material Design icon set.
#
# What is drawn, and where it goes, is what `app/desktop/icons.toml` says: this is the way to run the tool
# that reads it, so that a picture is drawn the same way from a terminal, from `export`, and on any machine
# the tree is checked out on. The tool draws its own pictures, so there is nothing here for the shell and its
# PowerShell twin to differ on.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# shellcheck disable=SC2086
$CARGO run --quiet -p rorolala-dev-icons
