#!/usr/bin/env bash
# Legacy: signs with the local self-signed Augmont identity (not Apple Developer ID).
# For internal distribution use: bash scripts/sign-macos-adhoc.sh
# (ad-hoc, Hardened Runtime, no fake Apple cert).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CERT_DIR="$ROOT/certs"
DIST="$ROOT/dist/macos"
SIGNING_OUT="$ROOT/dist/signing"
IDENTITY="390D682CF7D3571E5ED89784FFEA6A8E94BF6BF7"
IDENTITY_NAME="Augmont Goldtech"

mkdir -p "$DIST" "$SIGNING_OUT"
if [[ -f "$CERT_DIR/augmont-ca.crt" ]]; then
  cp "$CERT_DIR/augmont-ca.crt" "$SIGNING_OUT/Install-this-CA-on-PCs.crt"
  cp "$CERT_DIR/augmont-ca.crt" "$DIST/Install-this-CA-on-PCs.crt"
  cp "$ROOT/scripts/install-augmont-ca.ps1" "$SIGNING_OUT/install-augmont-ca.ps1"
fi

APP="$ROOT/src-tauri/target/aarch64-apple-darwin/release/bundle/macos/LGD FTP DATA SYNC.app"
if [[ ! -d "$APP" ]]; then
  WORKDIR="$(mktemp -d)"
  trap 'rm -rf "$WORKDIR"' EXIT
  ditto -x -k "$DIST/LGD FTP DATA SYNC.app.zip" "$WORKDIR"
  APP="$(find "$WORKDIR" -name '*.app' | head -1)"
fi
if [[ -z "${APP:-}" || ! -d "$APP" ]]; then
  echo "Could not find LGD FTP DATA SYNC.app"
  exit 1
fi

if ! security find-identity -v -p codesigning 2>/dev/null | grep -q "$IDENTITY"; then
  echo "Signing identity $IDENTITY ($IDENTITY_NAME) is not in the keychain."
  echo "Import certs/augmont-codesign.p12 first."
  exit 1
fi

KC="$CERT_DIR/augmont-codesign.keychain-db"
PASS_FILE="$CERT_DIR/PASSWORD.txt"
if [[ -f "$KC" && -f "$PASS_FILE" ]]; then
  PASS="$(cat "$PASS_FILE")"
  security unlock-keychain -p "$PASS" "$KC"
  security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$PASS" "$KC" >/dev/null
  echo "Signing with identity: $IDENTITY_NAME ($IDENTITY)"
  codesign --force --deep --sign "$IDENTITY" --keychain "$KC" --timestamp=none "$APP"
else
  echo "Signing with identity: $IDENTITY_NAME ($IDENTITY)"
  codesign --force --deep --sign "$IDENTITY" --timestamp=none "$APP"
fi
codesign --verify --deep --strict --verbose=2 "$APP"
codesign -dv --verbose=2 "$APP" 2>&1 | head -20

rm -f "$DIST/LGD FTP DATA SYNC.app.zip"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$DIST/LGD FTP DATA SYNC.app.zip"

export CODESIGN_IDENTITY="$IDENTITY"
DMG="$DIST/LGD FTP DATA SYNC_1.0.1_aarch64.dmg"
bash "$ROOT/scripts/make-macos-dmg.sh" "$APP" "$DMG"

echo "Self-signed macOS files:"
echo "  $DIST/LGD FTP DATA SYNC.app.zip"
echo "  $DMG"
echo "CA to install on PCs/Macs: $DIST/Install-this-CA-on-PCs.crt"
