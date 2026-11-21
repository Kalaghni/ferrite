#!/bin/sh
# Build FERRITE.app and its disk image from universal macOS binaries.
#
#   packaging/build-dmg.sh VERSION BIN_DIR OUT.dmg
#
# BIN_DIR holds ferrite, ferrite-server and ferrite-forge-mcp. The app is
# signed ad hoc (not notarised), so the first open goes through Privacy &
# Security. Inside the app the game keeps its files in
# ~/Library/Application Support/FERRITE (crates/shared/src/paths.rs).
set -eu

version="$1"
bins="$2"
dmg="$3"
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

stage="$work/FERRITE"
app="$stage/FERRITE.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
for b in ferrite ferrite-server ferrite-forge-mcp; do
  cp "$bins/$b" "$app/Contents/MacOS/$b"
  chmod +x "$app/Contents/MacOS/$b"
done
sed "s/@VERSION@/$version/g" "$here/Info.plist" > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"
cp "$here/ferrite.icns" "$app/Contents/Resources/ferrite.icns"

# The helpers first, then the app around them.
codesign --force --sign - "$app/Contents/MacOS/ferrite-server"
codesign --force --sign - "$app/Contents/MacOS/ferrite-forge-mcp"
codesign --force --sign - "$app"
codesign --verify --deep --strict "$app"

ln -s /Applications "$stage/Applications"
cp "$here/README.txt" "$stage/Read me.txt"
cp "$here/Quantico-OFL.txt" "$stage/"

rm -f "$dmg"
hdiutil create -volname "FERRITE $version" -srcfolder "$stage" -fs HFS+ -format UDZO -ov "$dmg"
echo "built $dmg"
