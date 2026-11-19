#!/bin/bash
# Build M2WGame and package it as a disk image (.dmg) for a GitHub Release.
# Usage: ./scripts/make-dmg.sh      -> dist/M2WGame-<version>-arm64.dmg (+ .sha256)
#
# The app is only ad-hoc signed (no Apple Developer ID), so macOS Gatekeeper warns on first
# launch. The README explains how to open it. Notarizing needs a paid Apple Developer account.
set -euo pipefail
cd "$(dirname "$0")/.."

./scripts/build-swift.sh

if [ -n "${CARGO_TARGET_DIR:-}" ]; then
  TARGET_DIR="$CARGO_TARGET_DIR"
elif [ -f .cargo/config.toml ] && grep -q 'target-dir' .cargo/config.toml; then
  TARGET_DIR="$(grep 'target-dir' .cargo/config.toml | head -1 | cut -d'"' -f2)"
else
  TARGET_DIR="target"
fi
APP="$TARGET_DIR/M2WGame.app"
VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
NAME="M2WGame-$VERSION-arm64"
STAGE="$TARGET_DIR/dmg-stage"
OUT="dist/$NAME.dmg"

codesign --verify --deep --strict "$APP"

rm -rf "$STAGE"
mkdir -p "$STAGE" dist
cp -R "$APP" "$STAGE/M2WGame.app"
ln -s /Applications "$STAGE/Applications"
cat > "$STAGE/READ ME FIRST.txt" <<TXT
M2WGame $VERSION

1. Drag M2WGame.app onto the Applications folder.
2. The first time, macOS may say the app "cannot be opened because the developer cannot be
   verified" (it is not notarized by Apple). Open it anyway:
   - Right-click (or Control-click) M2WGame.app, choose Open, then Open again; or
   - System Settings > Privacy & Security > scroll down > "Open Anyway".
   Or, in Terminal:  xattr -dr com.apple.quarantine /Applications/M2WGame.app
3. You need a Wine engine; see https://github.com/ryanle1203/m2wgame#get-an-engine
TXT

rm -f "$OUT"
hdiutil create -volname "M2WGame $VERSION" -srcfolder "$STAGE" -ov -format UDZO "$OUT" >/dev/null
( cd dist && shasum -a 256 "$NAME.dmg" > "$NAME.dmg.sha256" )
echo "Done: $OUT"
cat "dist/$NAME.dmg.sha256"
