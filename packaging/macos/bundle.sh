#!/usr/bin/env bash
# AsOne.app dla macOS z binarek release, spakowane do ZIP:
#   packaging/macos/bundle.sh <target> <arch>   np. aarch64-apple-darwin arm64
# Zmienne: EXTRA_DYLIBS – biblioteki do Contents/MacOS (Intel: libonnxruntime z PyPI),
#          MIN_MACOS – LSMinimumSystemVersion (domyślnie 11.0).
# Wynik: dist/AsOne-<wersja>-macos-<arch>.zip (podpis ad-hoc, bez notaryzacji Apple).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TARGET="$1"
ARCH="$2"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
REL="$ROOT/target/$TARGET/release"
WORK="$ROOT/target/macos-package/$ARCH"
APP="$WORK/AsOne.app"

rm -rf "$WORK"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$ROOT/dist"
cp "$REL/asone" "$APP/Contents/MacOS/AsOne"
cp "$REL/asone-cli" "$APP/Contents/MacOS/asone-cli"
for lib in ${EXTRA_DYLIBS:-}; do
  cp -L "$lib" "$APP/Contents/MacOS/"
done

# Ikona .icns z PNG 512 px.
SRC="$ROOT/packaging/linux/icons/hicolor/512x512/apps/io.github.lucasterg.AsOne.png"
ICONSET="$WORK/AsOne.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z "$s" "$s" "$SRC" --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  sips -z "$((s * 2))" "$((s * 2))" "$SRC" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AsOne.icns"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>AsOne</string>
  <key>CFBundleDisplayName</key><string>AsOne</string>
  <key>CFBundleIdentifier</key><string>io.github.lucasterg.AsOne</string>
  <key>CFBundleExecutable</key><string>AsOne</string>
  <key>CFBundleIconFile</key><string>AsOne</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>${MIN_MACOS:-11.0}</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.graphics-design</string>
</dict>
</plist>
PLIST

for lib in "$APP"/Contents/MacOS/*.dylib; do
  [ -e "$lib" ] && codesign --force --sign - "$lib"
done
codesign --force --deep --sign - "$APP"
OUT="$ROOT/dist/AsOne-$VERSION-macos-$ARCH.zip"
rm -f "$OUT"
ditto -c -k --keepParent "$APP" "$OUT"
echo "zapisano $OUT"
