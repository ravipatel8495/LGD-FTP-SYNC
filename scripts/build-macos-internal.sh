#!/usr/bin/env bash
# Build universal (Apple Silicon + Intel) app/DMG and ad-hoc sign for internal use.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script must run on macOS."
  exit 1
fi

if ! command -v rustup >/dev/null; then
  echo "Install Rust from https://rustup.rs"
  exit 1
fi

rustup target add aarch64-apple-darwin x86_64-apple-darwin
if [[ ! -d node_modules ]]; then
  npm ci
fi

echo "Building universal Mac app + DMG (ad-hoc identity '-')"
npx tauri build --target universal-apple-darwin --bundles dmg,app --config signing.macos.adhoc.json

TARGET_BASE="${CARGO_TARGET_DIR:-$ROOT/src-tauri/target}"
APP="$TARGET_BASE/universal-apple-darwin/release/bundle/macos/LGD FTP DATA SYNC.app"
if [[ ! -d "$APP" ]]; then
  APP="$ROOT/src-tauri/target/universal-apple-darwin/release/bundle/macos/LGD FTP DATA SYNC.app"
fi
if [[ ! -d "$APP" ]]; then
  echo "Build did not produce the .app under $TARGET_BASE or src-tauri/target"
  exit 1
fi

DMG="$(find "$TARGET_BASE/universal-apple-darwin/release/bundle/dmg" "$ROOT/src-tauri/target/universal-apple-darwin/release/bundle/dmg" -name '*.dmg' -type f 2>/dev/null | head -1 || true)"

if [[ -n "${DMG:-}" && -f "$DMG" ]]; then
  bash "$ROOT/scripts/sign-macos-adhoc.sh" "$APP" "$DMG"
  bash "$ROOT/scripts/verify-macos-signing.sh" "$APP" "$DMG"
else
  bash "$ROOT/scripts/sign-macos-adhoc.sh" "$APP"
  bash "$ROOT/scripts/verify-macos-signing.sh" "$APP"
fi

DIST="$ROOT/dist/macos"
mkdir -p "$DIST"
ditto "$APP" "$DIST/LGD FTP DATA SYNC.app"
rm -f "$DIST/LGD FTP DATA SYNC.app.zip"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$DIST/LGD FTP DATA SYNC.app.zip"
if [[ -n "${DMG:-}" && -f "$DMG" ]]; then
  cp -f "$DMG" "$DIST/"
fi

echo
echo "Internal Mac build ready:"
echo "  $DIST/LGD FTP DATA SYNC.app"
echo "  $DIST/LGD FTP DATA SYNC.app.zip"
[[ -n "${DMG:-}" ]] && echo "  $DIST/$(basename "$DMG")"
echo
echo "Install on another Mac: docs/macos-internal-distribution.md"
