#!/bin/bash
# Build QuickChat.app from a binary built with `--features quickchat`.
#
#   packaging/macos/quickchat/bundle.sh <binary> <output/QuickChat.app> <version>
#
# QuickChat has its own bundle identity, so it never replaces ZapFast.app or
# shares its notification permission. Set CODESIGN_IDENTITY to use a
# Developer ID; otherwise the bundle gets the ad-hoc signature arm64 needs.
set -euo pipefail

binary="$1"
app="$2"
version="$3"
numeric_version="${version%%-*}"
here="$(cd "$(dirname "$0")" && pwd)"
macos="$(dirname "$here")"

# Guard ZapFast's bundle against accidental replacement.
if [ "$(basename "$app")" != "QuickChat.app" ]; then
    echo "QuickChat bundles must be written to a QuickChat.app destination" >&2
    exit 2
fi
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

cp "$binary" "$app/Contents/MacOS/quickchat"
chmod 755 "$app/Contents/MacOS/quickchat"
sed "s/__VERSION__/$numeric_version/g" "$here/Info.plist" > "$app/Contents/Info.plist"

iconset="$(mktemp -d)/quickchat.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    sips -z $size $size "$macos/icon-1024.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z $double $double "$macos/icon-1024.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/quickchat.icns"

if [ -n "${CODESIGN_IDENTITY:-}" ]; then
    codesign --force --timestamp --options runtime \
        --entitlements "$macos/entitlements.plist" --sign "$CODESIGN_IDENTITY" "$app"
else
    codesign --force --entitlements "$macos/entitlements.plist" --sign - "$app"
fi
codesign --verify --strict "$app"

echo "$app"
