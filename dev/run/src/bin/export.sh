#!/bin/sh
# Lays the build out for hand-off under `$BUILD_DIR`, under the names a consumer links, includes and
# sources. That is also what puts the Desktop program where `rola desktop` reaches for it, so an
# export is what makes that command work.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# The plugin's own pictures first, and deliberately: they are drawn from the pinned icon set rather
# than carried, so what is laid out below is what the pin holds rather than whatever was last in the
# tree. It is the one step here that reaches outside the tree for its input.
again icons

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

# Every program is laid down beside the others, so that a word of `rola`'s reaches the program it
# names — `rola jj` is `rola-jj` — and the completions are laid down for the programs that have them.
for program in $PROGRAMS $PROGRAMS_WITHOUT_COMPLETIONS; do
	cp "$RELEASE_DIR/$program$EXE_SUFFIX" "$BUILD_DIR/bin/$program$EXE_SUFFIX"
done

for program in $PROGRAMS; do
	mkdir -p "$SCRIPTS_DIR/$program"
	for shell in $COMPLETION_SHELLS; do
		cp "$COMPLETION_DIR/${program}_comp.$shell" \
			"$SCRIPTS_DIR/$program/$program-completion.$shell"
	done
done

# The whole solution is restored and built once for everything below rather than a project at a time:
# the restore is shared, and one MSBuild over the solution is what builds the host, the plugins and the
# bindings together.
build_solution

publish_desktop "$DESKTOP_DIR"
publish_plugins

# The C# bindings beside the native library they bind, under `lib/` where a consumer of the C ABI
# looks for both.
publish_rsharp

# And the words that go with it: what the hand-off is, and what it is handed over under. They are the
# tree's own at its top rather than files written for a layout, because a second copy is a copy that
# goes stale — what an export carries is what the project says about itself, said once.
cp README.md LICENSE "$BUILD_DIR/"
