#!/usr/bin/env bash
# Builds the release tarball install.sh downloads, in dist/:
#
#   lanwell-<arch>-linux.tar.gz          the app, its helper, desktop entry and icons
#   lanwell-<arch>-linux.tar.gz.sha256   its checksum
#   install.sh                           the installer, for the release's assets
#   latest.json                          the version, which the app checks for updates
#
# The release workflow runs this on a tag; it works the same locally, from
# any folder, and ends by printing how to try install.sh on the result.

set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

arch="$(uname -m)"
pkgid="$(cargo pkgid -p lanwell)" # path+file:///…/crates/app#lanwell@0.1.0
version="${pkgid##*@}"
asset="lanwell-$arch-linux.tar.gz"

(cd crates/app && cargo tauri build --no-bundle --ci)
cargo build --release -p lanwell-helper

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
out="$stage/lanwell"
mkdir -p "$out/icons"

# The release profile keeps line tables for backtraces; the download doesn't need them.
strip -o "$out/lanwell" target/release/lanwell
strip -o "$out/lanwell-helper" target/release/lanwell-helper
cp crates/app/linux/lanwell.desktop LICENSE "$out/"
cp crates/app/icons/32x32.png "$out/icons/32.png"
cp crates/app/icons/128x128.png "$out/icons/128.png"
cp crates/app/icons/128x128@2x.png "$out/icons/256.png"
cp crates/app/icons/icon.png "$out/icons/512.png"
echo "$version" >"$out/VERSION"

rm -rf dist
mkdir dist
tar -C "$stage" -czf "dist/$asset" lanwell
(cd dist && sha256sum "$asset" >"$asset.sha256")
cp install.sh dist/
printf '{"version": "%s"}\n' "$version" >dist/latest.json

echo "Packaged lanwell $version:"
ls -l dist
echo
echo "To try it with install.sh, from any folder:"
echo "  LANWELL_RELEASE_URL=\"file://$repo/dist\" sh \"$repo/dist/install.sh\""
