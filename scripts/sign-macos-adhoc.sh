#!/usr/bin/env bash
# Ad-hoc codesign for internal/local Mac distribution (no Apple Developer ID).
# Signs nested Mach-O inside-out, then the .app, with Hardened Runtime.
# Usage: bash scripts/sign-macos-adhoc.sh [path-to.app] [path-to.dmg]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENTITLEMENTS="$ROOT/src-tauri/entitlements.plist"
IDENTITY="-"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script must run on macOS."
  exit 1
fi

if [[ ! -f "$ENTITLEMENTS" ]]; then
  echo "Missing entitlements: $ENTITLEMENTS"
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
  echo "No .app found. Build first: npm run dist:mac:internal"
  exit 1
fi

sign_macho_file() {
  local path="$1"
  codesign --force --options runtime --timestamp=none --sign "$IDENTITY" "$path"
}

# Deepest Mach-O files first so nested binaries are signed before the bundle.
echo "Signing nested Mach-O in: $APP"
find "$APP/Contents" -type f -print 2>/dev/null \
  | while IFS= read -r f; do
      if file -b "$f" 2>/dev/null | grep -q 'Mach-O'; then
        printf '%s\t%s\n' "${#f}" "$f"
      fi
    done \
  | sort -nr \
  | while IFS=$'\t' read -r _ path; do
      echo "  codesign (nested): $path"
      sign_macho_file "$path"
    done

echo "Signing app bundle with entitlements + Hardened Runtime"
codesign --force --options runtime --timestamp=none \
  --entitlements "$ENTITLEMENTS" \
  --sign "$IDENTITY" \
  "$APP"

echo "Ad-hoc signed: $APP"

DMG="${2:-}"
if [[ -n "$DMG" && -f "$DMG" ]]; then
  echo "Signing DMG: $DMG"
  codesign --force --timestamp=none --sign "$IDENTITY" "$DMG"
fi
