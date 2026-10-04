#!/bin/sh
# Draws the plugin's own pictures from the pinned Material Design icon set.
#
# The set is not vendored: it is a shallow clone under `THIRD-PARTY/`, which git ignores, and every
# picture is drawn from it rather than copied by hand, so what is drawn is what the pinned commit
# holds. What is drawn is committed, so a build needs neither the set nor a renderer — this script is
# what needs them, and `export` runs it so that what ships is drawn from the pin rather than from
# whatever was last in the tree.
#
# The clone is shallow and detached: one commit, no history, and the commit is the one named below.
# A clone left at another commit, or with anything changed in it, is put back, since it is a
# dependency and not a place to work.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# The commit the pictures are drawn from.
#
# It is the whole of what "the icon set" means here: there is no history to travel, so another set is
# another clone, and another picture is a line in `ICONS` below. The upstream is archived, so this is
# expected to stay where it is.
ICONS_COMMIT="af0ed9c0e1276bad43c4d6ca8e8aaa283e425195"
ICONS_URL="https://github.com/material-icons/material-icons"

# Where the set is kept, and where the pictures drawn from it live.
ICONS_CLONE="THIRD-PARTY/material-icons"
ICONS_PICTURES="app/desktop/Plugins/RorolalaVCSPlugin/icons"

# The pictures to draw, by the name the set knows each icon by.
#
# `lock` is what an entry somebody else holds is drawn with, and `create` — the set's pencil — what one
# the reader holds is drawn with, since holding it is what makes it theirs to change. Each is the
# `baseline` family, which is the set's own shape for the icon.
ICONS="lock create"

# What a picture is drawn in, and how many pixels wide and tall it is.
#
# The ink is white and the ground transparent, because the Desktop draws a picture as a mask over a
# colour it takes from the theme: white is opaque under either reading of a mask — its alpha, and its
# luminance — so the mark takes the colour the user is looking at rather than one baked into the file.
# The size is four times the set's own 24-unit grid, which is as large as a tile's icon ever gets.
ICONS_INK="#FFFFFF"
ICONS_SIZE=96

# Puts the clone where this expects it: the named commit, and nothing changed in it.
ensure_icons() {
	if [ ! -d "$ICONS_CLONE/.git" ]; then
		echo "==> cloning $ICONS_URL"
		rm -rf "$ICONS_CLONE"
		mkdir -p "$ICONS_CLONE"
		git -C "$ICONS_CLONE" init -q
		git -C "$ICONS_CLONE" remote add origin "$ICONS_URL"
	fi

	if [ "$(git -C "$ICONS_CLONE" rev-parse HEAD 2>/dev/null || true)" != "$ICONS_COMMIT" ]; then
		echo "==> checking out $ICONS_COMMIT"
		git -C "$ICONS_CLONE" fetch -q --depth 1 origin "$ICONS_COMMIT"
		git -C "$ICONS_CLONE" checkout -q --detach "$ICONS_COMMIT"
	fi

	if [ -n "$(git -C "$ICONS_CLONE" status --porcelain)" ]; then
		echo "==> restoring $ICONS_CLONE"
		git -C "$ICONS_CLONE" checkout -q --force "$ICONS_COMMIT"
		git -C "$ICONS_CLONE" clean -qffd
	fi
}

# Draws one picture, named as the set names it.
# $1 — the icon's name.
draw_icon() {
	source="$ICONS_CLONE/svg/$1/baseline.svg"
	picture="$ICONS_PICTURES/$1.png"

	if [ ! -f "$source" ]; then
		echo "the icon set holds no \`$1\` at $ICONS_COMMIT" >&2
		exit 1
	fi

	if ! command -v "$RSVG" >/dev/null 2>&1; then
		echo "no renderer: \`$RSVG\` is not on PATH — install librsvg, or name another with RSVG=" >&2
		exit 1
	fi

	# The set draws in black, so the ink is set on the way through rather than left to the renderer,
	# which has no colour of its own to be told. The whole is a subshell so that the inked copy is
	# taken away on the way out however the renderer ends, and so that the next icon starts clean.
	(
		inked=$(mktemp)
		trap 'rm -f "$inked"' EXIT

		sed "s/<path /<path fill=\"$ICONS_INK\" /g" "$source" > "$inked"

		# shellcheck disable=SC2086
		$RSVG -w "$ICONS_SIZE" -h "$ICONS_SIZE" -o "$picture" "$inked"
	)

	echo "==> $picture"
}

echo "==> icons at $ICONS_COMMIT"
ensure_icons

for icon in $ICONS; do
	draw_icon "$icon"
done
