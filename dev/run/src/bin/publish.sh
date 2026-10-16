#!/bin/sh
# Lays an export out and packages it, which is what a person is handed: what an export is made of is
# programs, and what someone who is not a developer wants is something that puts them where a shell and a
# file manager will find them.
#
# Two packages, because the two platforms do not agree on what one is. Windows is given an installer — a
# program that copies the layout, writes the two things that make a program reachable (the profile and the
# path) and registers the one that makes it convenient (the context menu), and that can be asked to undo all
# of it. Unix is given an archive and the script that installs it, since what a Unix hands over is a tree and
# a script and the packaging of them is the distribution's business rather than this project's.
set -eu

. "$(dirname "$0")/../lib/common.sh"

VERSION=$(workspace_version)
PUBLISHER=$(workspace_publisher)
PACKAGE_DIR="$BUILD_DIR"

# The archive a Unix is given, and the installer a Windows is given: both are named for what they are, the
# version they are of, and the platform they are for, since that is what tells two hand-offs apart on a disk
# that has both.
ARCHIVE="$PACKAGE_DIR/rorolala-$VERSION-$(uname -s | tr 'A-Z' 'a-z')-$(uname -m).tar.gz"

# The installer's own file name, which the script that builds it is told rather than naming out of its own
# head: what a package is called is decided here, beside the archive, so the two read alike.
SETUP="rorolala-$VERSION-setup"

export VERSION SETUP ARCHIVE

# The toolchain first, and deliberately: what an export costs is minutes of compiling, and a machine that has no
# compiler for the installer should say so before one is built for a package that cannot be made. Which one is
# used — this project's own under the cache, or the one on this machine's path — is what `require_iscc` answers
# with and says.
if [ -n "$EXE_SUFFIX" ]; then
	ISCC=$(require_iscc)
fi

again export

# The programs of a Unix install are the ones an archive holds, and what tells them where to go is the script
# beside them; a Windows install is a program, so it is built here.
if [ -n "$EXE_SUFFIX" ]; then
	# The compiler reads its sources out of the layout and writes the installer into the layout, so anything of
	# a previous build left there would be read as part of the payload: an export clears the directory first and
	# leaves nothing, and this is what makes that true even of a run that did not export.
	rm -f "$PACKAGE_DIR/$SETUP.exe"

	# The script is told the three things it cannot work out for itself: what version this is, what the
	# installer is to be called, and where the layout it packages was laid and is to be written — the last of
	# them as a path a Windows program can read, since this shell spells a path the way Unix does.
	#
	# The switches are written with two slashes, which is not a typo: the shells this runs in on Windows
	# translate a lone `/word` into a path before the program is started — `/DVersion=0.1.0` reaches it as
	# something beginning with a drive letter — and a leading `//` is what tells them not to. ISCC's own
	# switches are spelled with one, and it is given one.
	"$ISCC" //Q \
		"//DVersion=$VERSION" \
		"//DPublisher=$PUBLISHER" \
		"//DSetup=$SETUP" \
		"//DSourceDir=$(win_path "$PWD/$BUILD_DIR")" \
		"//DOutputDir=$(win_path "$PWD/$PACKAGE_DIR")" \
		"$(win_path "$PWD/dev/packaging/windows/rorolala.iss")"

	ls -l "$PACKAGE_DIR/$SETUP.exe"
else
	# The layout and the script that installs it, as one archive: what a Unix is given is the tree as it was
	# laid out, so nothing here changes it beyond adding the script a reader runs.
	cp dev/packaging/unix/install.sh "$BUILD_DIR/install.sh"
	chmod +x "$BUILD_DIR/install.sh"

	tar -czf "$ARCHIVE" -C "$BUILD_DIR" .
	ls -l "$ARCHIVE"
fi
