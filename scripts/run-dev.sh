#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
glib-compile-schemas --strict data
GSETTINGS_SCHEMA_DIR="$PWD/data" cargo run -- "$@"
