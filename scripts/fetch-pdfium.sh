#!/bin/sh
# Download the pdfium library used for PDF → image rendering into src-tauri/resources/pdfium/.
# The version and checksum are pinned; the binary is git-ignored. Run once after cloning:
#   sh scripts/fetch-pdfium.sh
set -eu

VERSION="chromium/8111"
case "$(uname -s)" in
  Darwin)
    ASSET="pdfium-mac-univ.tgz" # Apple Silicon + Intel
    SHA256="043638ac3d7830ff43a7cdbf0623e6b73d70d5cbff9e12e9531d5be7e03b8e9d"
    LIB="lib/libpdfium.dylib"
    ;;
  *)
    echo "fetch-pdfium: only macOS is set up so far (Windows arrives with the Windows port)." >&2
    exit 1
    ;;
esac

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/src-tauri/resources/pdfium"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Downloading $ASSET ($VERSION)…"
curl -fsSL "https://github.com/bblanchon/pdfium-binaries/releases/download/$VERSION/$ASSET" -o "$TMP/$ASSET"
echo "$SHA256  $TMP/$ASSET" | shasum -a 256 -c -

tar -xzf "$TMP/$ASSET" -C "$TMP" "$LIB" LICENSE licenses
rm -rf "$DEST"
mkdir -p "$DEST"
cp "$TMP/$LIB" "$DEST/"
cp "$TMP/LICENSE" "$DEST/LICENSE-pdfium-binaries.txt"
cp -R "$TMP/licenses" "$DEST/licenses"
echo "pdfium installed in src-tauri/resources/pdfium/"
