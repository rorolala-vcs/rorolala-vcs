#!/bin/sh
# Runs the integration suites under `tests/`. Each is a program of its own — deliberately not a
# member of the workspace — that runs the built programs and checks what they said, so what is
# checked is the program a caller meets rather than the parts it is made of.
#
# A suite is a program rather than a test crate, because what it does is what a caller does: it is
# run, and it complains — by ending non-zero — when what came back was not what it expected. That is
# also what lets one do what a test cannot: serve a Vault, reach it, and stop it again.
#
# The suites are a workspace of their own, so they are run from beside it: where a suite is run from
# is what picks its target directory out of `tests/.cargo/config.toml`, which is how running them
# leaves nothing inside the workspace they test. The programs they run are the release ones this
# script builds, named to them rather than looked for.
#
# They run one at a time, so a suite that fails is named rather than lost among the output of the
# others — and the ones after it still run, so one run reports every suite that failed.
set -eu

. "$(dirname "$0")/../lib/common.sh"

again bin

# The programs are named to a suite by where they are, which has to be said before the suites are
# entered: from here on, everything is relative to `tests/`.
bin_dir="$(pwd)/$RELEASE_DIR"

cd tests

failed=
for suite in */; do
	[ -f "$suite/Cargo.toml" ] || continue

	echo "==> $suite"
	# shellcheck disable=SC2086
	if ! ROLA_BIN_DIR="$bin_dir" $CARGO run --manifest-path "$suite/Cargo.toml"; then
		failed="$failed $suite"
	fi
done

if [ -n "$failed" ]; then
	echo "==> failed:$failed"
	exit 1
fi
