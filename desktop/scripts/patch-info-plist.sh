#!/usr/bin/env bash
# Patch LSUIElement=true into every smallcast.app Info.plist under
# src-tauri/target/. Without this key the bundle is treated as a regular
# dock app — even with NSWindow level=screensaver and CanJoinAllSpaces,
# macOS still won't float the palette over another app's fullscreen Space.
# Tauri 2's bundle config has no infoPlist override, so we patch after.
set -euo pipefail

cd "$(dirname "$0")/.."

shopt -s nullglob
plists=(
  src-tauri/target/release/bundle/macos/smallcast.app/Contents/Info.plist
  src-tauri/target/*-apple-darwin/release/bundle/macos/smallcast.app/Contents/Info.plist
)

found=0
for plist in "${plists[@]}"; do
  [ -f "$plist" ] || continue
  found=1
  plutil -insert LSUIElement -bool YES "$plist" 2>/dev/null \
    || plutil -replace LSUIElement -bool YES "$plist"
  echo "patched LSUIElement: $plist"
  # The patch invalidates Tauri's signature; a stable identity keeps macOS permissions across rebuilds.
  if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
    codesign --force --preserve-metadata=entitlements,requirements,flags,runtime \
      --sign "$APPLE_SIGNING_IDENTITY" "$(dirname "$(dirname "$plist")")"
  fi
done

if [ "$found" = "0" ]; then
  echo "patch-info-plist.sh: no smallcast.app bundles found under target/ — nothing to patch" >&2
fi
