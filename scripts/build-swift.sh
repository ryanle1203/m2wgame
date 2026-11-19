#!/bin/bash
# Build the SwiftUI M2WGame.app: Rust core (static library) + Swift UI, bundled and ad-hoc signed.
# Usage: ./scripts/build-swift.sh   (from anywhere)
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

# Where cargo writes its output: $CARGO_TARGET_DIR, else .cargo/config.toml's target-dir, else "target".
if [ -n "${CARGO_TARGET_DIR:-}" ]; then
  TARGET_DIR="$CARGO_TARGET_DIR"
elif [ -f .cargo/config.toml ] && grep -q 'target-dir' .cargo/config.toml; then
  TARGET_DIR="$(grep 'target-dir' .cargo/config.toml | head -1 | cut -d'"' -f2)"
else
  TARGET_DIR="target"
fi
APP="$TARGET_DIR/M2WGame.app"
OBJ="$TARGET_DIR/swift-build"

echo "1/3 Rust core..."
cargo build --release --lib

echo "2/3 SwiftUI..."
mkdir -p "$OBJ"
swiftc -O -parse-as-library -swift-version 5 \
  -target arm64-apple-macos14.0 \
  -import-objc-header swift/Sources/bridge.h \
  swift/Sources/*.swift \
  -L "$TARGET_DIR/release" -lm2wgame_core \
  -o "$OBJ/M2WGame"

echo "3/3 App bundle..."
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$OBJ/M2WGame" "$APP/Contents/MacOS/M2WGame"
[ -f swift/AppIcon.icns ] && cp swift/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>M2WGame</string>
  <key>CFBundleDisplayName</key><string>M2WGame</string>
  <key>CFBundleIdentifier</key><string>io.github.ryanle1203.m2wgame</string>
  <key>CFBundleExecutable</key><string>M2WGame</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>MIT License. M2WGame contributors.</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.games</string>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$APP"
echo "Done: $APP"
