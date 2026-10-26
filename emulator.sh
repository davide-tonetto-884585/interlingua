#!/usr/bin/env bash
# Rebuilds the plugin for the computer and restarts the desktop emulator
# with it. Usage: ./emulator.sh [profile]   (default: xteink-x4-pro;
# `paperpaw-emu --list-profiles` has the others).
set -euo pipefail

cd "$(dirname "$0")"
PROFILE="${1:-xteink-x4-pro}"
EMU="${PAPERPAW_EMU:-$HOME/paperpaw-emu}"
LIB="$EMU/library/plugins/interlingua"

cargo build --release -p interlingua-host

mkdir -p "$LIB"
cp plugin/plugin.ini "$LIB/"
rm -rf "$LIB/how" && cp -R plugin/how "$LIB/how"
cp target/release/libinterlingua_host.dylib "$LIB/code-host.dylib"

# Close the emulator that is still open, if any, so the new build loads.
pkill -x paperpaw-emu 2>/dev/null && sleep 1 || true

cd "$EMU"
exec ./bin/paperpaw-emu --profile "$PROFILE" --library library --trust-code
