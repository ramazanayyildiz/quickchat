#!/bin/bash
# Build an optimized QuickChat.app, sign it, and put it in place.
#
#   packaging/macos/quickchat/release.sh [destination folder]
#
# The destination defaults to ~/Applications. Signs with CODESIGN_IDENTITY,
# else the keychain's first "Developer ID Application" identity, else ad
# hoc. A stable signature keeps the Keychain's and the notification
# permission's approval across rebuilds; an ad-hoc one asks again each time.
# Quit a running QuickChat first: the bundle is replaced in place.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
destination="${1:-$HOME/Applications}"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"

if pgrep -fq "QuickChat.app/Contents/MacOS/quickchat"; then
    echo "QuickChat is running: quit it from its menu-bar icon first" >&2
    exit 1
fi

cd "$root"
cargo build --locked --release --features quickchat

if [ -z "${CODESIGN_IDENTITY:-}" ]; then
    CODESIGN_IDENTITY="$(security find-identity -v -p codesigning |
        sed -n 's/.*"\(Developer ID Application: [^"]*\)".*/\1/p' | head -1)"
    export CODESIGN_IDENTITY
fi
if [ -n "$CODESIGN_IDENTITY" ]; then
    echo "Signing with $CODESIGN_IDENTITY"
else
    echo "No Developer ID found: signing ad hoc"
fi

mkdir -p "$destination"
"$here/bundle.sh" "$root/target/release/zapfast" "$destination/QuickChat.app" "$version"
