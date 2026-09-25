#!/bin/sh
#
# What every script here needs: where the toolchains are, where their output goes, and how this
# project spells the artifacts of a platform.
#
# This file is sourced, never run, and it is not under `.run/src/bin/` — so the runner does not offer
# it as a script. It is what the scripts are made of, not one of them.

# The runner itself, for a script that is several scripts. A `make` target that was several targets
# said so in its prerequisites; a script here says so by asking the runner for the others.
RUN=./run.sh

# The toolchains. Override them to pass extra flags, e.g. `CARGO="cargo --offline" ./run.sh export`.
#
# They are used unquoted everywhere below, deliberately: a toolchain that is several words has to be
# split into those words before it is run, and quoting it would make it one word that is not a
# command.
CARGO="${CARGO:-cargo}"
DOTNET="${DOTNET:-dotnet}"

# Cargo's target directory, matching `.cargo/config.toml`.
TARGET_DIR="${TARGET_DIR:-.cache/rs-target}"

# .NET's output directory, matching `Directory.Build.props`. It is under the same `.cache` as
# cargo's, so the two toolchains' output sits in one place.
CS_TARGET_DIR="${CS_TARGET_DIR:-.cache/cs-target}"

# The directory `export` lays the hand-off artifacts out in.
BUILD_DIR="${BUILD_DIR:-build}"

# The only profile anything here builds. Everything it produces is release, so nothing that ships
# can carry debug assertions or debug info.
RELEASE_FLAG=--release

# Where cargo writes the release artifacts.
RELEASE_DIR="$TARGET_DIR/release"

# The generated C header, written by the root build script while the library builds.
HEADER="$RELEASE_DIR/ffi_bindings/rorolala_ffi.h"

# Completion scripts. While the programs compile, mingling's `gen_program!()` writes one per
# supported shell into this directory — deliberately not profile-specific, so the same files serve
# every build.
COMPLETION_DIR="${COMPLETION_DIR:-$TARGET_DIR/mingling}"

# Shells mingling generates a completion script for.
COMPLETION_SHELLS="sh zsh fish ps1"

# The command line programs. Each is a workspace member whose package name is also its binary name,
# so one list drives the build, the export, and the completion scripts — mingling names those
# `<program>_comp.<shell>` and they are exported as
# `scripts/<program>/<program>-completion.<shell>`. The scripts call their command by name and never
# refer to their own file name, so the exported name is free.
PROGRAMS="rola rola-daemon"

# The .NET side of the workspace as one solution: the Desktop program, the translations it reads,
# and the tests of those translations.
SOLUTION=RorolalaSharp.sln

# Where an export puts the completion scripts: one directory per program, so what a shell's setup
# sources is named once per program rather than once per program and shell.
SCRIPTS_DIR="$BUILD_DIR/scripts"

# The Desktop program: the project it is built from, and where an export lays it out. It is laid out
# in a `desktop` directory of its own beside the command line programs, which is where
# `rola desktop` reaches for it — so an export and that command name one place.
DESKTOP_PROJECT=app/desktop/Core/RorolalaDesktop.csproj
DESKTOP_DIR="$BUILD_DIR/bin/desktop"

# The plugins that ship with the Desktop program. Each is a project of its own, built apart from the
# program that discovers it, and laid out only when something is run or handed over. The name of the
# directory holding the project is also the assembly's name, which is what says which files to take
# from the output it was built into.
DESKTOP_PLUGINS="app/desktop/plugins/FileSystemPlugin"

# Asks the runner for another script, the way a `make` target asked for another target.
again() {
	"$RUN" "$@"
}

# Publishes the Desktop program into the directory named. `dotnet publish` is the form that produces
# a program with everything beside it that running it needs, which is what an export holds; a plain
# build leaves a program whose dependencies are still only in the build tree.
publish_desktop() {
	# shellcheck disable=SC2086
	$DOTNET publish "$DESKTOP_PROJECT" -c Release -o "$1"
}

# Builds each plugin that ships with the Desktop program and lays it where the program looks for it:
# its assembly and its translations under `plugins/` beside the program. Only those are taken. The
# contract and Avalonia the plugin was built against are the host's own copies, delegated to rather
# than carried, so laying them down would be laying down a second of each.
publish_plugins() {
	for project in $DESKTOP_PLUGINS; do
		name=$(basename "$project")
		output="$CS_TARGET_DIR/$name/bin/Release/net8.0"

		# shellcheck disable=SC2086
		$DOTNET build "$project/$name.csproj" -c Release

		mkdir -p "$DESKTOP_DIR/plugins"
		cp "$output/$name.dll" "$DESKTOP_DIR/plugins/"

		if [ -d "$output/i18n" ]; then
			cp -r "$output/i18n" "$DESKTOP_DIR/plugins/"
		fi
	done
}

# `export` and `clean` delete a destination with `rm -rf`, so refuse a path that would take
# something else with it.
guard() {
	case "$2" in
	/*)
		echo "$1 must be a relative path, got \`$2'" >&2
		exit 1
		;;
	'')
		echo "$1 must not be empty" >&2
		exit 1
		;;
	esac
}

guard BUILD_DIR "$BUILD_DIR"
guard TARGET_DIR "$TARGET_DIR"
guard CS_TARGET_DIR "$CS_TARGET_DIR"

# Platform spellings. Cargo names a Unix shared library with a `lib` prefix and a Windows one with a
# `.dll` suffix, so the exported names are normalised to one shape per platform:
#
#   | cargo artifact               | exported as                       |
#   | ---------------------------- | --------------------------------- |
#   | librorolala.so               | $BUILD_DIR/lib/rorolala.so        |
#   | librorolala.dylib            | $BUILD_DIR/lib/rorolala.dylib     |
#   | rorolala.dll                 | $BUILD_DIR/lib/rorolala.dll       |
#   | rorolala.dll.lib             | $BUILD_DIR/lib/rorolala.dll.lib   |
#   | librorolala.a                | $BUILD_DIR/lib/rorolala.a         |
#   | rorolala.lib                 | $BUILD_DIR/lib/rorolala.lib       |
#   | ffi_bindings/rorolala_ffi.h  | $BUILD_DIR/lib/rorolala.h         |
#   |                              | $BUILD_DIR/lib/rorolala.hpp       |
#   | rola[.exe]                   | $BUILD_DIR/bin/rola[.exe]         |
#   | mingling/<program>_comp.*    | $BUILD_DIR/scripts/<program>/*    |
#
# The header is C and C++ at once — its declarations sit in an `extern "C"` block — so `.hpp` is the
# same file under the name a C++ project includes. The completion scripts are the four shells
# mingling generates: sh, zsh, fish and ps1, each program's written into `$SCRIPTS_DIR/<program>/` as
# `<program>-completion.<shell>`.
#
# The import library is the one artifact only Windows has: a DLL is linked through it, and the static
# library of the same crate needs nothing beside it, which is why the two `.lib` files are both
# handed over and named apart.
case "${OS:-}" in
Windows_NT)
	EXE_SUFFIX=.exe
	SHARED_SUFFIX=dll
	STATIC_SUFFIX=lib
	CARGO_SHARED="rorolala.$SHARED_SUFFIX"
	CARGO_STATIC="rorolala.$STATIC_SUFFIX"
	# A DLL is linked through the import library that names what it exports, and cargo writes that
	# one under the DLL's own name — `rorolala.dll.lib` — so that it does not take the static
	# library's.
	CARGO_IMPORT="rorolala.$SHARED_SUFFIX.$STATIC_SUFFIX"
	IMPORT_SUFFIX="$SHARED_SUFFIX.$STATIC_SUFFIX"
	;;
*)
	EXE_SUFFIX=
	SHARED_SUFFIX=so
	case "$(uname -s)" in
	Darwin) SHARED_SUFFIX=dylib ;;
	esac
	STATIC_SUFFIX=a
	CARGO_SHARED="librorolala.$SHARED_SUFFIX"
	CARGO_STATIC="librorolala.$STATIC_SUFFIX"
	# Nothing to hand over: a linker is given the shared library itself.
	CARGO_IMPORT=
	IMPORT_SUFFIX=
	;;
esac
