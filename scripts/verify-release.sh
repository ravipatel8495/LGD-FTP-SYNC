#!/usr/bin/env bash
# Verify a downloaded installer against SHA256SUMS.txt and GitHub attestations.
# Usage: bash scripts/verify-release.sh path/to/installer [path/to/SHA256SUMS.txt]
set -euo pipefail

FILE="${1:-}"
SUMS="${2:-SHA256SUMS.txt}"
REPO="${VERIFY_REPO:-ravipatel8495/LGD-FTP-SYNC}"

if [[ -z "$FILE" || ! -f "$FILE" ]]; then
  echo "Usage: bash scripts/verify-release.sh <installer> [SHA256SUMS.txt]"
  exit 1
fi

if [[ -f "$SUMS" ]]; then
  NAME="$(basename "$FILE")"
  EXPECTED="$(awk -v n="$NAME" '$2==n || $2=="*"n {print $1}' "$SUMS" | head -1)"
  ACTUAL="$(shasum -a 256 "$FILE" | awk '{print $1}')"
  if [[ -z "$EXPECTED" ]]; then
    echo "No checksum for $NAME in $SUMS"
    exit 1
  fi
  if [[ "$EXPECTED" != "$ACTUAL" ]]; then
    echo "SHA-256 mismatch for $NAME"
    echo "expected $EXPECTED"
    echo "got      $ACTUAL"
    exit 1
  fi
  echo "SHA-256 OK: $NAME"
else
  echo "No $SUMS — skip checksum (download it from the GitHub Release)"
fi

if command -v gh >/dev/null 2>&1; then
  gh attestation verify "$FILE" --repo "$REPO"
else
  echo "Install GitHub CLI (gh) to verify provenance: https://cli.github.com"
fi
