#!/bin/sh
# Removes everything the build ever produced: cargo's output, whatever else `$TARGET_DIR` holds (the
# generated header, mingling's completion scripts), .NET's output, and the exported `$BUILD_DIR`.
set -eu

. "$(dirname "$0")/../lib/common.sh"

again cargo-clean

rm -rf "$TARGET_DIR" "$CS_TARGET_DIR" "$BUILD_DIR"
