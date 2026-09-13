#!/usr/bin/env bash
# Build the AppImage from the contents of the .deb, leaving WebKitGTK out of
# it (platform/linux/webkit-excludes.sh explains why it has to stay out).
#
# `dx bundle` builds an AppImage of its own, but it calls linuxdeploy with a
# fixed set of arguments and no way to pass `--exclude-library`, so it can only
# produce the image that carries WebKitGTK. The AppImage is therefore built
# here, from the payload dx already assembled for the .deb — the same
# executable, resources, desktop entry and icon an AppDir needs — with the same
# linuxdeploy release dx would have used.
set -euo pipefail

usage="usage: platform/linux/build-appimage.sh <path to .deb> <output .AppImage>"
deb="${1:?$usage}"
output="${2:?$usage}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

case "$(uname -m)" in
  aarch64 | arm64) arch="aarch64" ;;
  x86_64) arch="x86_64" ;;
  *)
    echo "Error: no linuxdeploy release for $(uname -m)" >&2
    exit 1
    ;;
esac

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT

appdir="$workdir/AppDir"
mkdir -p "$appdir"
dpkg-deb --fsys-tarfile "$deb" | tar -x -C "$appdir"

executable="$appdir/usr/bin/arto"
desktop="$appdir/usr/share/applications/arto.desktop"
# The icon is not handed to linuxdeploy: `--icon-file` makes it parse the image
# with CImg, which rejects this PNG ("libpng error: Read Error"). It is already
# at the hicolor path linuxdeploy would have deployed it to, and the root
# symlink linuxdeploy writes is found from the desktop entry's Icon= by name,
# without reading the image.
icon="$(find "$appdir/usr/share/icons" -name 'arto.png' -print -quit)"
if [[ ! -f "$executable" || ! -f "$desktop" || ! -f "$icon" ]]; then
  echo "Error: $deb is missing the executable, desktop entry or icon an AppDir needs" >&2
  exit 1
fi

# Cached outside the bundle directory: everything left there matching
# *.AppImage is picked up as a release artifact.
linuxdeploy="${XDG_CACHE_HOME:-$HOME/.cache}/arto/linuxdeploy-$arch.AppImage"
if [[ ! -x "$linuxdeploy" ]]; then
  mkdir -p "$(dirname "$linuxdeploy")"
  curl --proto '=https' --tlsv1.2 --location --silent --show-error --fail \
    --output "$linuxdeploy" \
    "https://github.com/tauri-apps/binary-releases/releases/download/linuxdeploy/linuxdeploy-$arch.AppImage"
  chmod +x "$linuxdeploy"
fi

excludes=()
while IFS= read -r library; do
  excludes+=(--exclude-library "$library")
done < <("$here/webkit-excludes.sh")

# dx no longer creates the AppImage, so nothing has made this directory.
mkdir -p "$(dirname "$output")"

# `OUTPUT` is how linuxdeploy's AppImage plugin is told where to write.
# Extract-and-run because CI runners and containers frequently have no
# /dev/fuse, and no stripping because the binaries are already release builds.
OUTPUT="$output" APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=true "$linuxdeploy" \
  --appdir "$appdir" \
  --executable "$executable" \
  --desktop-file "$desktop" \
  "${excludes[@]}" \
  --output appimage

echo "AppImage created at $output"
