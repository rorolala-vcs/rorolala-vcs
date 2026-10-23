#!/bin/sh
# Installs Rorolala where a shell and a desktop will find it, and can be asked to take it back.
#
# What a Unix is handed is a tree and a script rather than a package: the layout `cargo dev export` lays down
# is what is installed, and this is the script that puts it there. It does the four things that make a
# program reachable rather than merely present — copies the tree, writes a `bin` directory into the path, lays
# the completion scripts where the shell sources them, and registers a desktop entry — and each of them is
# written conservatively, so that removing it removes what this script wrote and nothing else.
#
#   install.sh [--prefix DIR] [--uninstall] [--no-path] [--no-completions] [--no-desktop-entry]
#
# The prefix is where the whole thing goes, `~/.local` by default: what is installed is one person's, so what
# it is installed into is one person's too.
set -eu

PREFIX="${PREFIX:-$HOME/.local}"
HERE=$(cd "$(dirname "$0")" && pwd)

DO_PATH=yes
DO_COMPLETIONS=yes
DO_ENTRY=yes
UNINSTALL=no

while [ $# -gt 0 ]; do
	case "$1" in
	--prefix)
		shift
		PREFIX=$1
		;;
	--uninstall) UNINSTALL=yes ;;
	--no-path) DO_PATH=no ;;
	--no-completions) DO_COMPLETIONS=no ;;
	--no-desktop-entry) DO_ENTRY=no ;;
	*)
		echo "install.sh: what is \`$1'?" >&2
		exit 2
		;;
	esac
	shift
done

APPS="$PREFIX/share/rorolala"
BIN="$PREFIX/bin"
COMPLETIONS="$PREFIX/share"
ENTRY="$PREFIX/share/applications"
DESKTOP="rorolala-desktop"

# What is written into the shell's own startup file, between two markers that say it is ours: a block whose
# absence means there is nothing to take back, and whose presence is the whole of what an uninstall needs to
# find.
PROFILE_BEGIN="# Rola begin"
PROFILE_END="# Rola end"
PROFILE="${ROLA_PROFILE:-}"

# Which file the shell reads when it starts. Read from the environment where it is set, and worked out from
# the shell's own name where it is not: this is the one place a script like this has to guess, so it guesses
# only the common shells and says nothing where it does not know.
if [ -z "$PROFILE" ]; then
	case "$(basename "${SHELL:-sh}")" in
	bash) PROFILE="$HOME/.bashrc" ;;
	zsh) PROFILE="$HOME/.zshrc" ;;
	fish) PROFILE="$HOME/.config/fish/config.fish" ;;
	*) PROFILE="" ;;
	esac
fi

say() { printf '%s\n' "$*"; }

# Removes the block this script wrote, and keeps a copy of what stood there before it.
#
# Conservative in the one way that matters: a file without both markers is a file this script did not write
# into, and it is left exactly as it stands.
drop_block() {
	[ -n "$PROFILE" ] || return 0
	[ -f "$PROFILE" ] || return 0

	grep -q "^$PROFILE_BEGIN\$" "$PROFILE" 2>/dev/null || return 0
	grep -q "^$PROFILE_END\$" "$PROFILE" 2>/dev/null || return 0

	[ -f "$PROFILE.rorolala-backup" ] || cp "$PROFILE" "$PROFILE.rorolala-backup"

	awk -v begin="$PROFILE_BEGIN" -v end="$PROFILE_END" '
		$0 == begin { inside = 1; next }
		$0 == end { inside = 0; next }
		!inside { print }
	' "$PROFILE" > "$PROFILE.rorolala-new"

	mv "$PROFILE.rorolala-new" "$PROFILE"
}

write_block() {
	[ -n "$PROFILE" ] || return 0

	drop_block

	mkdir -p "$(dirname "$PROFILE")"
	touch "$PROFILE"

	{
		printf '\n%s\n' "$PROFILE_BEGIN"
		for script in "$COMPLETIONS/rola/rola-completion.sh" "$COMPLETIONS/rola-daemon/rola-daemon-completion.sh"; do
			[ -f "$script" ] && printf '. "%s"\n' "$script"
		done
		printf 'export PATH="%s:$PATH"\n' "$BIN"
		printf '# Uncomment to choose the editor a run opens its edits in, and the language it speaks:\n'
		printf '# export ROLA_EDITOR="rola-edit"\n'
		printf '# export ROLA_LANG="en"\n'
		printf '%s\n' "$PROFILE_END"
	} >> "$PROFILE"
}

drop_completions() {
	[ "$DO_COMPLETIONS" = yes ] || return 0

	rm -f "$COMPLETIONS/rola/rola-completion.sh" "$COMPLETIONS/rola/rola-completion.fish" \
		"$COMPLETIONS/rola/rola-completion.zsh"
	rm -f "$COMPLETIONS/rola-daemon/rola-daemon-completion.sh" \
		"$COMPLETIONS/rola-daemon/rola-daemon-completion.fish" \
		"$COMPLETIONS/rola-daemon/rola-daemon-completion.zsh"
	rmdir "$COMPLETIONS/rola" "$COMPLETIONS/rola-daemon" 2>/dev/null || true
}

write_completions() {
	[ "$DO_COMPLETIONS" = yes ] || return 0

	mkdir -p "$COMPLETIONS/rola" "$COMPLETIONS/rola-daemon"

	# What the shells source: the same script under the names each of them looks for, since a completion
	# script is one file and where it is read from is the shell's answer rather than this layout's.
	for program in rola rola-daemon; do
		for shell in sh zsh fish; do
			[ -f "$APPS/scripts/$program/$program-completion.$shell" ] || continue
			cp "$APPS/scripts/$program/$program-completion.$shell" \
				"$COMPLETIONS/$program/$program-completion.$shell"
		done
	done
}

drop_entry() {
	[ "$DO_ENTRY" = yes ] || return 0

	rm -f "$ENTRY/$DESKTOP.desktop"
}

write_entry() {
	[ "$DO_ENTRY" = yes ] || return 0

	mkdir -p "$ENTRY"

	# Opened the way the context menu opens it on Windows, through `rola desktop`: what finds the Workspace
	# and hands the window the command line it works with is the command, not the program.
	cat > "$ENTRY/$DESKTOP.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Rorolala Desktop
Comment=Reality-Oriented, Layout-Agnostic
Exec=$BIN/rola desktop
Icon=$APPS/res/icons/yizi.png
Terminal=false
Categories=Development;
EOF
}

if [ "$UNINSTALL" = yes ]; then
	drop_entry
	drop_completions
	drop_block
	rm -rf "$APPS"

	say "Rorolala was removed from $PREFIX."
	say "A copy of your shell's startup file, from before this project first wrote into it, is at:"
	[ -n "$PROFILE" ] && say "  $PROFILE.rorolala-backup"
	exit 0
fi

# What is installed is the whole layout, taken as it stands: the programs, the libraries beside them, the
# Desktop program and the scripts. Nothing here rearranges it, so what is installed is what was exported.
mkdir -p "$APPS"
cp -R "$HERE/." "$APPS/"

write_completions

# The block is what puts the directory on the path as well as sourcing the completions: one block, one set of
# markers, and one thing to take back out, which is what makes the uninstall something that cannot
# half-happen.
if [ "$DO_PATH" = yes ]; then
	write_block
fi

write_entry

say "Rorolala was installed under $APPS."
say "The command line is at $BIN/rola, and the Desktop program is opened by \`rola desktop'."
[ "$DO_PATH" = yes ] && [ -n "$PROFILE" ] && say "Start a new shell, or source $PROFILE, to have it on the path."
