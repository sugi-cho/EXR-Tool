#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/../apps/exrtool-gui/src-tauri"

cargo tauri build --bundles dmg

DMG="$(find ../target/release/bundle/dmg -name '*.dmg' | head -n 1 || true)"
if [[ -n "${APPLE_ID:-}" && -n "${APPLE_PASSWORD:-}" && -n "${APPLE_TEAM_ID:-}" && -n "$DMG" ]]; then
  xcrun notarytool submit "$DMG" --apple-id "$APPLE_ID" --password "$APPLE_PASSWORD" --team-id "$APPLE_TEAM_ID" --wait
  xcrun stapler staple "$DMG"
else
  echo "Skipping notarization, missing APPLE_ID/APPLE_PASSWORD/APPLE_TEAM_ID or dmg" >&2
fi
