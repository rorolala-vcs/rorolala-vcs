#!/bin/sh
# Checks that every i18n key a call site names is stated by the translation files it reads: the
# `t!("...")`s of the command line programs, and the `I18n.Get("...")`s of the Desktop host and its
# plugins. A key written as anything but a string literal is counted rather than guessed at, since
# what it adds up to is not known until the program runs.
set -eu

. "$(dirname "$0")/../lib/common.sh"

# shellcheck disable=SC2086
$CARGO run --quiet -p rorolala-dev-i18n-check
