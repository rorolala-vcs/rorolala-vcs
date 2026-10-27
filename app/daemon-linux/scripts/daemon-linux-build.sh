#!/bin/sh
# Builds the GTK4 window over the daemon into an export.
#
# It is reached by `cargo dev export` on Unix and can be run on its own. Like its
# Windows twin it is self-contained: it finds the tree from its own place, and
# `BUILD_DIR` is the same override the build honours.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
source_dir="$root/app/daemon-linux"
src_dir="$source_dir/src"

case "${BUILD_DIR:-}" in
"") build_root="$root/build" ;;
/*) build_root="$BUILD_DIR" ;;
*) build_root="$root/$BUILD_DIR" ;;
esac

bin_dir="$build_root/bin"
obj_root="$root/.cache/daemon-linux"
obj_dir="$obj_root/obj"
exe="$bin_dir/rola-daemon-gui"

CC=${CC:-cc}
PKG_CONFIG=${PKG_CONFIG:-pkg-config}
check=no

if [ "${1:-}" = "--check" ]; then
	check=yes
fi

if [ ! -d "$src_dir" ]; then
	printf '%s\n' "Daemon sources were not found at $src_dir." >&2
	exit 1
fi

if ! command -v "$CC" >/dev/null 2>&1; then
	printf '%s\n' "A C compiler was not found; set CC to name one." >&2
	exit 1
fi

if ! command -v "$PKG_CONFIG" >/dev/null 2>&1; then
	printf '%s\n' "pkg-config was not found; it is what finds GTK 4." >&2
	exit 1
fi

if ! "$PKG_CONFIG" --exists gtk4; then
	printf '%s\n' "The GTK 4 development files were not found (pkg-config gtk4)." >&2
	exit 1
fi

# GtkAlertDialog and GtkFileDialog are 4.10, and they are what keeps the dialogs
# off the deprecated chooser and message box.
if ! "$PKG_CONFIG" --atleast-version=4.10 gtk4; then
	printf '%s\n' "GTK 4.10 or newer is required; this machine has $("$PKG_CONFIG" --modversion gtk4)." >&2
	exit 1
fi

cflags=$("$PKG_CONFIG" --cflags gtk4)
libs=$("$PKG_CONFIG" --libs gtk4)

# POSIX process and signal declarations, which a strict C11 run hides by default.
defines="-D_POSIX_C_SOURCE=200809L"
# GTK's own headers are not pedantic C (they carry declarations that end in an
# extra semicolon), so -Wpedantic would fail inside them rather than here.
warnings="-std=c11 -O2 -Wall -Wextra -Werror -Wshadow -Wformat=2 -Wstrict-prototypes -Wmissing-prototypes -Wwrite-strings"

if [ "$check" = yes ]; then
	# The stricter analyses are GCC's; another compiler is still checked by the
	# warnings above, which are the ones it shares.
	case "$("$CC" --version 2>/dev/null)" in
	*GCC*)
		warnings="$warnings -Wduplicated-cond -Wduplicated-branches -Wlogical-op -Wnull-dereference"
		;;
	esac
fi

mkdir -p "$bin_dir" "$obj_dir"
rm -f "$obj_dir"/*.o

for source in "$src_dir"/*.c; do
	object="$obj_dir/$(basename "${source%.c}").o"
	# shellcheck disable=SC2086
	"$CC" $defines $warnings $cflags -c "$source" -o "$object"
done

# shellcheck disable=SC2086
"$CC" $warnings -o "$exe" "$obj_dir"/*.o $libs

printf 'Built %s\n' "$exe"

# The program looks for rola-daemon beside itself, so an export lays that
# program down first. A plain build may run before that, and says so rather than
# failing: building the C program does not depend on the Rust output.
if [ ! -f "$bin_dir/rola-daemon" ]; then
	printf '%s\n' "warning: rola-daemon is not in $bin_dir yet; put it beside the GUI before running it." >&2
fi

exit 0
