#!/bin/sh
set -eu
repo=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
build=${WARD_BUILD_DIR:-"$repo/target/debian-package"}
output=${WARD_OUTPUT_DIR:-"$repo/dist"}
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repo/Cargo.toml" | head -n 1)
architecture=$(dpkg --print-architecture)
mkdir -p "$build" "$output"
cargo build --manifest-path "$repo/Cargo.toml" --target-dir "$build/cargo" --release --locked --offline
stage=$(mktemp -d "$build/ward-stage.XXXXXX")
trap 'rm -rf "$stage"' EXIT
WARD_LICENSE_DIR=/usr/share/doc/ward "$repo/packaging/stage.sh" "$build/cargo/release" "$stage"
strip --strip-unneeded "$stage/usr/bin/ward" "$stage/usr/libexec/ward-helper"
mkdir -p "$build/debian" "$stage/DEBIAN"
cp "$repo/packaging/debian/control" "$build/debian/control"
cd "$build"
dependencies=$(dpkg-shlibdeps -O -e"$stage/usr/bin/ward" -e"$stage/usr/libexec/ward-helper" | sed -n 's/^shlibs:Depends=//p')
[ -n "$dependencies" ]
python3 - "$repo" "$stage" "$version" "$architecture" "$dependencies" <<'PY'
from pathlib import Path
import sys
repo, stage, version, architecture, dependencies = sys.argv[1:]
stage = Path(stage)
source = Path(repo, 'packaging/debian/control').read_text().split('\n\nPackage: ward\n', 1)[1]
source = source.replace('Architecture: any', f'Architecture: {architecture}')
source = source.replace('${shlibs:Depends}', dependencies)
maintainer = next(line for line in Path(repo, 'packaging/debian/control').read_text().splitlines() if line.startswith('Maintainer:'))
size = sum(p.stat().st_size for p in stage.rglob('*') if p.is_file()) // 1024
(stage/'DEBIAN/control').write_text(f'Package: ward\nVersion: {version}-1\nSection: net\nPriority: optional\n{maintainer}\nInstalled-Size: {size}\n' + source)
PY
install -m755 "$repo/packaging/debian/postinst" "$stage/DEBIAN/postinst"
install -m755 "$repo/packaging/debian/postrm" "$stage/DEBIAN/postrm"
install -m644 "$repo/packaging/debian/copyright" "$stage/usr/share/doc/ward/copyright"
(cd "$stage" && find usr -type f -exec md5sum '{}' + > DEBIAN/md5sums)
glib-compile-schemas --strict --dry-run "$stage/usr/share/glib-2.0/schemas"
desktop-file-validate "$stage/usr/share/applications/io.github.ward.Ward.desktop"
dpkg-deb --root-owner-group --build "$stage" "$output/ward_${version}-1_${architecture}.deb"
