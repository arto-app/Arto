#!/bin/bash
# Generate platform-specific icons from SVG source
#
# Requirements:
#   - rsvg-convert (librsvg) or Inkscape for SVG to PNG
#   - ImageMagick (convert) for PNG to ICO
#
# Usage:
#   ./scripts/generate-icons.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
SVG_SOURCE="$ROOT_DIR/extras/arto.svg"

# Output directories
WINDOWS_DIR="$ROOT_DIR/extras/windows"
LINUX_DIR="$ROOT_DIR/extras/linux"

mkdir -p "$WINDOWS_DIR" "$LINUX_DIR"

# Generate PNG for Linux (256x256)
echo "Generating Linux icon..."
if command -v rsvg-convert &> /dev/null; then
    rsvg-convert -w 256 -h 256 "$SVG_SOURCE" -o "$LINUX_DIR/arto-app.png"
elif command -v inkscape &> /dev/null; then
    inkscape -w 256 -h 256 "$SVG_SOURCE" -o "$LINUX_DIR/arto-app.png"
else
    echo "Error: rsvg-convert or inkscape required for SVG to PNG conversion"
    exit 1
fi
echo "Created: $LINUX_DIR/arto-app.png"

# Generate ICO for Windows (multi-resolution)
echo "Generating Windows icon..."
if command -v convert &> /dev/null; then
    # Create multiple sizes for ICO
    TEMP_DIR=$(mktemp -d)
    for size in 16 32 48 64 128 256; do
        if command -v rsvg-convert &> /dev/null; then
            rsvg-convert -w $size -h $size "$SVG_SOURCE" -o "$TEMP_DIR/icon-$size.png"
        else
            inkscape -w $size -h $size "$SVG_SOURCE" -o "$TEMP_DIR/icon-$size.png"
        fi
    done
    convert "$TEMP_DIR/icon-16.png" "$TEMP_DIR/icon-32.png" "$TEMP_DIR/icon-48.png" \
            "$TEMP_DIR/icon-64.png" "$TEMP_DIR/icon-128.png" "$TEMP_DIR/icon-256.png" \
            "$WINDOWS_DIR/arto-app.ico"
    rm -rf "$TEMP_DIR"
    echo "Created: $WINDOWS_DIR/arto-app.ico"
else
    echo "Error: ImageMagick (convert) required for ICO generation"
    exit 1
fi

echo "Done!"
