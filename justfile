default: install

# Path to the auto-updater signing key. createUpdaterArtifacts is on, so
# every release build (local OR CI) needs this. Override on the command
# line if you keep the key elsewhere: `just SIGNING_KEY=~/keys/dc.key install`.
SIGNING_KEY := env_var_or_default("TAURI_SIGNING_PRIVATE_KEY_PATH", "~/.tauri/smallcast.key")

# Build smallcast.app and install it to /Applications.
install:
    @command -v pnpm  >/dev/null || { echo "pnpm required — brew install pnpm"; exit 1; }
    @command -v cargo >/dev/null || { echo "Rust required — https://rustup.rs"; exit 1; }
    @test -f {{SIGNING_KEY}} || { \
        echo "Missing updater signing key: {{SIGNING_KEY}}"; \
        echo "Generate one with: pnpm --dir desktop tauri signer generate --ci -p '' -w {{SIGNING_KEY}} -f"; \
        echo "(see README §Auto-update — keep the key in 1Password.)"; \
        exit 1; \
    }
    cd desktop && pnpm install --frozen-lockfile
    # The `pnpm tauri` wrapper (desktop/scripts/tauri.sh) patches Info.plist
    # with LSUIElement=true after every build — required to float over
    # another app's fullscreen Space.
    cd desktop && TAURI_SIGNING_PRIVATE_KEY="$(cat {{SIGNING_KEY}})" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" pnpm tauri build
    # The bundled binary is named `desktop` (CFBundleExecutable), so
    # `killall smallcast` is a no-op; match by full path instead.
    -pkill -f /Applications/smallcast.app 2>/dev/null || true
    rm -rf /Applications/smallcast.app
    cp -R desktop/src-tauri/target/release/bundle/macos/smallcast.app /Applications/
    xattr -cr /Applications/smallcast.app
    @echo ""
    @echo "✓ installed: /Applications/smallcast.app"
    @echo "  open it once to grant Accessibility, then use ⌃ Space anywhere."

# Vite + Rust hot-reload dev session.
dev:
    cd desktop && pnpm install
    cd desktop && pnpm tauri dev

# Build the release .app without copying it.
build:
    @test -f {{SIGNING_KEY}} || { echo "Missing updater signing key: {{SIGNING_KEY}}"; exit 1; }
    cd desktop && pnpm install --frozen-lockfile
    cd desktop && TAURI_SIGNING_PRIVATE_KEY="$(cat {{SIGNING_KEY}})" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" pnpm tauri build

# Remove smallcast from /Applications. Store at ~/Library/Application Support/smallcast/ is kept.
uninstall:
    # The bundled binary is named `desktop` (CFBundleExecutable), so
    # `killall smallcast` is a no-op; match by full path instead.
    -pkill -f /Applications/smallcast.app 2>/dev/null || true
    rm -rf /Applications/smallcast.app
    @echo "✓ removed /Applications/smallcast.app"

# Render the promo MP4 (1920x1080, 12s) to marketing/out/promo.mp4. All-synthetic data.
promo:
    cd marketing && pnpm install --frozen-lockfile
    cd marketing && pnpm render
    @echo ""
    @echo "✓ rendered: marketing/out/promo.mp4"

# Open the Remotion Studio for live-editing the promo composition.
promo-studio:
    cd marketing && pnpm install
    cd marketing && pnpm studio
