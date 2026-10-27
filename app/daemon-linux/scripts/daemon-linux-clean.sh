#!/bin/sh
# Removes this program's artifacts only: the export directory is shared with the
# Rust programs, and they are not this script's to remove.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)

case "${BUILD_DIR:-}" in
"") build_root="$root/build" ;;
/*) build_root="$BUILD_DIR" ;;
*) build_root="$root/$BUILD_DIR" ;;
esac

rm -rf "$root/.cache/daemon-linux"
rm -f "$build_root/bin/rola-daemon-gui"

printf '%s\n' "Cleaned daemon-linux build outputs."
