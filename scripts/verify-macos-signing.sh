#!/usr/bin/env bash
# Verify ad-hoc / Hardened Runtime signature on the .app (and optional .dmg).
# Usage: bash scripts/verify-macos-signing.sh [path-to.app] [path-to.dmg]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script must run on macOS."
  exit 1
fi

find_default_app() {
  local candidates=(
    "${CARGO_TARGET_DIR:-}/universal-apple-darwin/release/bundle/macos/LGD FTP DATA SYNC.app"
    "$ROOT/src-tauri/target/universal-apple-darwin/release/bundle/macos/LGD FTP DATA SYNC.app"
    "$ROOT/src-tauri/target/aarch64-apple-darwin/release/bundle/macos/LGD FTP DATA SYNC.app"
    "$ROOT/src-tauri/target/x86_64-apple-darwin/release/bundle/macos/LGD FTP DATA SYNC.app"
    "$ROOT/src-tauri/target/release/bundle/macos/LGD FTP DATA SYNC.app"
    "$ROOT/dist/macos/LGD FTP DATA SYNC.app"
  )
  local p
  for p in "${candidates[@]}"; do
    if [[ -d "$p" ]]; then
      printf '%s' "$p"
      return 0
    fi
  done
  return 1
}

APP="${1:-}"
if [[ -z "$APP" ]]; then
  APP="$(find_default_app || true)"
fi
if [[ -z "$APP" || ! -d "$APP" ]]; then
  echo "No .app found."
  exit 1
fi

echo "=== codesign --verify --deep --verbose=2 ==="
codesign --verify --deep --verbose=2 "$APP"

echo
echo "=== codesign -dv --verbose=4 (expect Signature=adhoc, flags=runtime) ==="
codesign -dv --verbose=4 "$APP" 2>&1

echo
echo "=== entitlements ==="
codesign -d --entitlements :- "$APP" 2>/dev/null || true

echo
echo "=== architectures ==="
lipo -info "$APP/Contents/MacOS/"* 2>/dev/null || true

echo
echo "=== spctl (ad-hoc internal apps are expected to fail Gatekeeper assessment) ==="
if spctl --assess --type execute -vv "$APP" 2>&1; then
  echo "spctl accepted the app."
else
  echo "spctl rejected (normal for ad-hoc). Use the per-app open steps in docs/macos-internal-distribution.md"
  echo "Do not run sudo spctl --master-disable."
fi

DMG="${2:-}"
if [[ -n "$DMG" && -f "$DMG" ]]; then
  echo
  echo "=== DMG ==="
  codesign -dv --verbose=2 "$DMG" 2>&1 || echo "DMG is unsigned or ad-hoc"
fi

echo
echo "Verification finished: $APP"
