#!/usr/bin/env bash
# Build a Finder DMG whose window size matches the background (no scrollbars)
# and whose icons sit on the gold arrow, not on the instructions.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_SRC="${1:-}"
VOLNAME="LGD FTP DATA SYNC"
OUT="${2:-$ROOT/dist/macos/LGD FTP DATA SYNC_1.0.1_universal.dmg}"
BG="$ROOT/build/dmg-background.png"

# Window is 660x428 so Finder's title bar does not clip the 660x400 artwork.
WIN_W=660
WIN_H=428
APP_X=180
APP_Y=250
DROP_X=480
DROP_Y=250
ICON_SIZE=96

if [[ -z "$APP_SRC" ]]; then
  TMPUNZIP="$(mktemp -d)"
  ditto -x -k "$ROOT/dist/macos/LGD FTP DATA SYNC.app.zip" "$TMPUNZIP"
  APP_SRC="$(find "$TMPUNZIP" -name '*.app' -maxdepth 3 | head -1)"
fi
if [[ -z "$APP_SRC" || ! -d "$APP_SRC" ]]; then
  echo "Need a .app — pass the path or keep dist/macos/*.app.zip"
  exit 1
fi

if [[ ! -f "$BG" ]]; then
  python3 "$ROOT/scripts/make-dmg-background.py"
fi

STAGE="$(mktemp -d)"
RWDIR="$(mktemp -d)"
RW="$RWDIR/rw.dmg"
MOUNT=""
cleanup() {
  if [[ -n "${MOUNT:-}" ]]; then
    hdiutil detach "$MOUNT" >/dev/null 2>&1 || true
  fi
  hdiutil detach "/Volumes/$VOLNAME" >/dev/null 2>&1 || true
  rm -rf "$STAGE" "$RWDIR" ${TMPUNZIP:-}
}
trap cleanup EXIT

ditto "$APP_SRC" "$STAGE/$VOLNAME.app"
ln -s /Applications "$STAGE/Applications"
mkdir -p "$STAGE/.background"
cp "$BG" "$STAGE/.background/background.png"

hdiutil create -volname "$VOLNAME" -srcfolder "$STAGE" -ov -format UDRW -fs HFS+ "$RW" >/dev/null
hdiutil detach "/Volumes/$VOLNAME" >/dev/null 2>&1 || true
hdiutil attach -readwrite -noverify -noautoopen "$RW" >/dev/null
MOUNT="/Volumes/$VOLNAME"
if [[ ! -d "$MOUNT" ]]; then
  echo "Failed to mount $RW at $MOUNT"
  exit 1
fi

# Make Finder pick up the background file
mkdir -p "$MOUNT/.background"
cp "$BG" "$MOUNT/.background/background.png"

# Blessing: hide helper files
if command -v SetFile >/dev/null 2>&1; then
  SetFile -a V "$MOUNT/.background" || true
fi

osascript <<EOF
tell application "Finder"
  tell disk "$VOLNAME"
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    try
      set sidebar width of container window to 0
    end try
    set the bounds of container window to {200, 120, $((200 + WIN_W)), $((120 + WIN_H))}
    set theViewOptions to the icon view options of container window
    set arrangement of theViewOptions to not arranged
    set icon size of theViewOptions to $ICON_SIZE
    set background picture of theViewOptions to file ".background:background.png"
    delay 0.4
    set position of item "$VOLNAME.app" of container window to {$APP_X, $APP_Y}
    set position of item "Applications" of container window to {$DROP_X, $DROP_Y}
    update without registering applications
    delay 1
    close
    open
    delay 1
    close
  end tell
end tell
EOF

sync
hdiutil detach "$MOUNT" >/dev/null
mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"
hdiutil convert "$RW" -format UDZO -imagekey zlib-level=9 -o "$OUT" >/dev/null
codesign --force --sign "${CODESIGN_IDENTITY:-390D682CF7D3571E5ED89784FFEA6A8E94BF6BF7}" "$OUT" 2>/dev/null \
  || codesign --force --sign - "$OUT" 2>/dev/null || true
echo "DMG ready: $OUT"
ls -lh "$OUT"
