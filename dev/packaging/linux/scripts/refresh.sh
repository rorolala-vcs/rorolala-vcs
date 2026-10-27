#!/bin/sh
# Refreshes the two caches a package writes into when its `.desktop` files or icons change.
#
# It deliberately says nothing about the daemon: `rolad.service` is a user service, and whether it
# runs is its user's decision. The package neither starts nor stops it.
#
# It is also the body of Arch's `.INSTALL` functions, which the package manager sources rather than
# runs: no `set -e`, no `exit`, and every command says what a missing tool means.

if command -v update-desktop-database >/dev/null 2>&1; then
	update-desktop-database -q /usr/share/applications || true
fi

if command -v gtk-update-icon-cache >/dev/null 2>&1; then
	gtk-update-icon-cache -q -t /usr/share/icons/hicolor || true
fi
