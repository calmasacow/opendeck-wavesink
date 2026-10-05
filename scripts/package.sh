#!/usr/bin/env bash
# Build the plugin and pack it the way OpenDeck installs it: one zip holding a
# single us.echo.wavesink.sdPlugin/ directory.
set -euo pipefail
cd "$(dirname "$0")/.."

target=x86_64-unknown-linux-gnu
plugin=us.echo.wavesink.sdPlugin
out=dist/$plugin

cargo build --release --locked --target "$target"
rm -rf dist
mkdir -p "$out/$target/bin"
cp -r assets/. "$out/"
cp "target/$target/release/opendeck-wavesink" "$out/$target/bin/"
cp LICENSE.md README.md "$out/"
(cd dist && zip -qr "$plugin.zip" "$plugin" && sha256sum "$plugin.zip" > SHA256SUMS)
echo "dist/$plugin.zip"
