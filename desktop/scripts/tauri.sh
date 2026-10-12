#!/usr/bin/env bash
# Wrap the Tauri CLI so every `pnpm tauri build` automatically patches
# Info.plist with LSUIElement=true. See patch-info-plist.sh for the why.
# `dev` and other subcommands exec straight through.
set -e

here="$(cd "$(dirname "$0")" && pwd)"
tauri="$here/../node_modules/.bin/tauri"

if [ "${1:-}" = "build" ]; then
  # Stable signature so macOS keeps Accessibility/folder grants across rebuilds.
  export APPLE_SIGNING_IDENTITY="${APPLE_SIGNING_IDENTITY:-Apple Development: Mariano Jalif (46VT2ZNYF9)}"
  # Don't let `set -e` skip the plist patch when `tauri build` exits non-zero
  # for a reason that still produced a bundle — most commonly the updater
  # signing step ("no private key") failing on a local build. The .app is
  # already on disk at that point and MUST get LSUIElement, or it loses its
  # menu-bar-utility behaviour (no float over fullscreen Spaces).
  set +e
  "$tauri" "$@"
  code=$?
  bash "$here/patch-info-plist.sh"
  exit "$code"
else
  exec "$tauri" "$@"
fi
