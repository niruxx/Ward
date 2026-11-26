#!/bin/sh
# Run after cargo build --release. This script installs files; it never changes firewall state.
set -eu
cd "$(dirname "$0")/.."
root="${DESTDIR:-}"
install -Dm755 target/release/ward "$root/usr/bin/ward"
install -Dm755 target/release/ward-helper "$root/usr/libexec/ward-helper"
install -Dm644 data/io.github.ward.Ward.policy "$root/usr/share/polkit-1/actions/io.github.ward.Ward.policy"
install -Dm644 data/io.github.ward.Ward.desktop "$root/usr/share/applications/io.github.ward.Ward.desktop"
install -Dm644 data/io.github.ward.Ward.gschema.xml "$root/usr/share/glib-2.0/schemas/io.github.ward.Ward.gschema.xml"
install -Dm644 data/io.github.ward.Ward.svg "$root/usr/share/icons/hicolor/scalable/apps/io.github.ward.Ward.svg"
if [ -z "$root" ]; then glib-compile-schemas /usr/share/glib-2.0/schemas; fi
