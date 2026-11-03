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
  MINGW*|MSYS*|CYGWIN*)
    ASSET="pdfium-win-x64.tgz" # Windows x64 (also runs on Windows on ARM)
    SHA256="3452d11031050ec65e57f180215b57b9e6d465e3fbedafa8a77ad90885c7d23e"
    LIB="bin/pdfium.dll"
    ;;
  *)
    echo "fetch-pdfium: only macOS and Windows (Git Bash) are set up." >&2
    exit 1
    ;;
esac

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/src-tauri/resources/pdfium"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Downloading $ASSET ($VERSION)…"
curl -fsSL "https://github.com/bblanchon/pdfium-binaries/releases/download/$VERSION/$ASSET" -o "$TMP/$ASSET"
if command -v sha256sum >/dev/null 2>&1; then
  echo "$SHA256  $TMP/$ASSET" | sha256sum -c -
else
  echo "$SHA256  $TMP/$ASSET" | shasum -a 256 -c -
fi

tar -xzf "$TMP/$ASSET" -C "$TMP" "$LIB" LICENSE licenses
rm -rf "$DEST"
mkdir -p "$DEST"
cp "$TMP/$LIB" "$DEST/"
cp "$TMP/LICENSE" "$DEST/LICENSE-pdfium-binaries.txt"
cp -R "$TMP/licenses" "$DEST/licenses"
echo "pdfium installed in src-tauri/resources/pdfium/"
