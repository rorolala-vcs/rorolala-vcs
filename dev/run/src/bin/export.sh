#!/bin/sh
# Lays the build out for hand-off under `$BUILD_DIR`, under the names a consumer links, includes and
# sources. That is also what puts the Desktop program where `rola desktop` reaches for it, so an
# export is what makes that command work.
set -eu

. "$(dirname "$0")/../lib/common.sh"

again build

rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR/lib" "$BUILD_DIR/bin"

cp "$RELEASE_DIR/$CARGO_SHARED" "$BUILD_DIR/lib/rorolala.$SHARED_SUFFIX"
cp "$RELEASE_DIR/$CARGO_STATIC" "$BUILD_DIR/lib/rorolala.$STATIC_SUFFIX"

# Only Windows has an import library, and only a DLL is handed over beside one.
if [ -n "$CARGO_IMPORT" ]; then
	cp "$RELEASE_DIR/$CARGO_IMPORT" "$BUILD_DIR/lib/rorolala.$IMPORT_SUFFIX"
fi

# The header is C and C++ at once, so it is handed over under both names.
cp "$HEADER" "$BUILD_DIR/lib/rorolala.h"
cp "$HEADER" "$BUILD_DIR/lib/rorolala.hpp"

for program in $PROGRAMS; do
	cp "$RELEASE_DIR/$program$EXE_SUFFIX" "$BUILD_DIR/bin/$program$EXE_SUFFIX"

	mkdir -p "$SCRIPTS_DIR/$program"
	for shell in $COMPLETION_SHELLS; do
		cp "$COMPLETION_DIR/${program}_comp.$shell" \
			"$SCRIPTS_DIR/$program/$program-completion.$shell"
	done
done

publish_desktop "$DESKTOP_DIR"
publish_plugins
