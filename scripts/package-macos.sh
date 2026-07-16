#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

cargo build --release -p evezip-ui
./scripts/fetch-7zz.sh

APP="target/EveZip.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp packaging/macos/Info.plist "$APP/Contents/"
cp target/release/evezip "$APP/Contents/MacOS/evezip"
cp vendor/7zz/macos/7zz "$APP/Contents/MacOS/7zz"   # localizado via "diretório do executável"
codesign --force --deep --sign - "$APP"

hdiutil create -volname EveZip -srcfolder "$APP" -ov -format UDZO target/EveZip.dmg
echo "ok: target/EveZip.dmg"
