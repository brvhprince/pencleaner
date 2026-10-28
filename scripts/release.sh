#!/bin/bash
# Builds, signs, verifies and installs PenCleaner.app.
#
#   scripts/release.sh            build + install to /Applications
#   scripts/release.sh --no-install
#   scripts/release.sh --dmg      also make a .dmg
#
# Signing identity: $PENCLEANER_SIGNING_IDENTITY, or the SHA-1 in .signing-identity
# (gitignored). List candidates with: security find-identity -v -p codesigning
# Use a stable identity (e.g. your Apple Development certificate): macOS ties Full Disk
# Access to the signature, so ad-hoc builds would lose it on every rebuild.
set -euo pipefail
cd "$(dirname "$0")/.."

install=1; bundles=app
for a in "$@"; do
  case "$a" in
    --no-install) install=0 ;;
    --dmg) bundles=app,dmg ;;
    *) echo "unknown option $a"; exit 2 ;;
  esac
done

id="${PENCLEANER_SIGNING_IDENTITY:-$(cat .signing-identity 2>/dev/null || true)}"
if [ -z "$id" ]; then
  echo "No signing identity. Put a certificate's SHA-1 in .signing-identity. Candidates:"
  security find-identity -v -p codesigning
  exit 1
fi
if ! security find-identity -v -p codesigning | grep -q "$id"; then
  echo "Signing identity $id isn't a valid code-signing identity in your keychain (expired?)."
  security find-identity -v -p codesigning
  exit 1
fi

echo "==> Building (release, signed with $id)…"
APPLE_SIGNING_IDENTITY="$id" pnpm tauri build --bundles "$bundles"

app="target/release/bundle/macos/PenCleaner.app"
echo "==> Verifying signature"
codesign --verify --deep --strict "$app"
codesign -dv --verbose=2 "$app" 2>&1 | grep -E "^(Authority=Apple Development|TeamIdentifier|Identifier)" || true
codesign -d --entitlements - "$app" 2>/dev/null | grep -q allow-unsigned-executable-memory \
  || { echo "Missing entitlement: Protection would crash. See src-tauri/Entitlements.plist"; exit 1; }

echo "==> Self-test (Protection's engine under the hardened runtime)"
"$app/Contents/MacOS/pencleaner" --self-test

if [ "$install" = 1 ]; then
  if pgrep -f "/Applications/PenCleaner.app/Contents/MacOS/pencleaner" >/dev/null; then
    echo "PenCleaner is running from /Applications. Quit it, then run this again."; exit 1
  fi
  echo "==> Installing to /Applications"
  if [ -d /Applications/PenCleaner.app ]; then
    # Keep the previous build recoverable.
    mv /Applications/PenCleaner.app "$HOME/.Trash/PenCleaner $(date +%Y%m%d-%H%M%S).app"
  fi
  ditto "$app" /Applications/PenCleaner.app
  echo "Installed /Applications/PenCleaner.app"
fi
