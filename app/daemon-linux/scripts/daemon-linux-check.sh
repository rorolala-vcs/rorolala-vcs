#!/bin/sh
# The C program is checked by the same build with the stricter warnings on and
# warnings as errors.
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

exec "$script_dir/daemon-linux-build.sh" --check
