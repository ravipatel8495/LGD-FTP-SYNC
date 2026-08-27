#!/usr/bin/env bash
# Remove quarantine from this app or DMG only (not a global Gatekeeper disable).
# Usage: bash scripts/macos-remove-quarantine.sh /path/to/LGD\ FTP\ DATA\ SYNC.app
set -euo pipefail

TARGET="${1:-}"
if [[ -z "$TARGET" || ! -e "$TARGET" ]]; then
  echo "Usage: bash scripts/macos-remove-quarantine.sh /path/to/app-or-dmg"
  exit 1
fi

xattr -d com.apple.quarantine "$TARGET" 2>/dev/null || true
xattr -cr "$TARGET"
echo "Cleared quarantine on: $TARGET"
echo "Now right-click the app → Open → Open (one time on this Mac)."
