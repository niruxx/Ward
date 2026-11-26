#!/bin/sh
# Stage files only. Never run cache updates or firewall operations here.
set -eu
repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
binaries="$1"
destination="$2"
install -Dm755 "$binaries/ward" "$destination/usr/bin/ward"
install -Dm755 "$binaries/ward-helper" "$destination/usr/libexec/ward-helper"
for file in io.github.ward.Ward.desktop; do
    install -Dm644 "$repo/data/$file" "$destination/usr/share/applications/$file"
done
install -Dm644 "$repo/data/io.github.ward.Ward.policy" "$destination/usr/share/polkit-1/actions/io.github.ward.Ward.policy"
install -Dm644 "$repo/data/io.github.ward.Ward.gschema.xml" "$destination/usr/share/glib-2.0/schemas/io.github.ward.Ward.gschema.xml"
install -Dm644 "$repo/data/io.github.ward.Ward.svg" "$destination/usr/share/icons/hicolor/scalable/apps/io.github.ward.Ward.svg"
install -Dm644 "$repo/LICENSE" "$destination${WARD_LICENSE_DIR:-/usr/share/licenses/ward}/LICENSE"
install -Dm644 "$repo/README.md" "$destination/usr/share/doc/ward/README.md"

install -Dm644 "$repo/THIRD-PARTY-NOTICES" "$destination/usr/share/doc/ward/THIRD-PARTY-NOTICES"
