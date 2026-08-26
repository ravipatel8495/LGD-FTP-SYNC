#!/usr/bin/env bash
# Pack Install-this-CA-on-PCs.crt into an existing .dmg (inside the .app and on the volume).
set -euo pipefail

DMG="${1:-}"
CRT="${2:-}"
if [[ -z "$DMG" || -z "$CRT" || ! -f "$DMG" || ! -f "$CRT" ]]; then
  echo "Usage: embed-ca-in-dmg.sh <installer.dmg> <Install-this-CA-on-PCs.crt>"
  exit 1
fi

WORKDIR="$(mktemp -d)"
RW="$WORKDIR/rw.dmg"
MOUNT=""
cleanup() {
  if [[ -n "${MOUNT:-}" ]]; then
    hdiutil detach "$MOUNT" -quiet >/dev/null 2>&1 || true
  fi
  rm -rf "$WORKDIR"
}
trap cleanup EXIT

hdiutil convert "$DMG" -format UDRW -o "$RW" -quiet
# attach returns mount point on last line
ATTACH_OUT="$(hdiutil attach -readwrite -noverify -noautoopen "$RW")"
MOUNT="$(echo "$ATTACH_OUT" | grep -o '/Volumes/.*' | tail -1 | tr -d '\r')"
if [[ -z "$MOUNT" || ! -d "$MOUNT" ]]; then
  echo "Could not mount $RW"
  exit 1
fi

cp "$CRT" "$MOUNT/Install-this-CA-on-PCs.crt"
APP="$(find "$MOUNT" -maxdepth 1 -name '*.app' | head -1)"
if [[ -n "$APP" && -d "$APP/Contents/Resources" ]]; then
  cp "$CRT" "$APP/Contents/Resources/Install-this-CA-on-PCs.crt"
fi

hdiutil detach "$MOUNT" -quiet
MOUNT=""
OUT="$WORKDIR/packed.dmg"
hdiutil convert "$RW" -format UDZO -imagekey zlib-level=9 -o "$OUT" -quiet
mv -f "$OUT" "$DMG"
echo "Packed CA into $DMG"
