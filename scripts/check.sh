#!/usr/bin/env bash
# Runs what CI runs (.github/workflows/ci.yml), on this Mac, before you commit.
#
#   scripts/check.sh           everything
#   scripts/check.sh --no-ui   skip the UI type-check and build
#
# Same Rust as CI (rust-toolchain.toml). For Windows it lints the code and builds the
# engine's test program with cargo-xwin; it also runs it when Wine is installed
# (`brew install --cask wine-stable`). Without Wine, Windows-only tests (registry,
# Recycle Bin, Windows paths) still only run on CI.
set -euo pipefail
cd "$(dirname "$0")/.."

ui=1
[[ "${1:-}" == "--no-ui" ]] && ui=0

step() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }
fail() { printf '\n\033[31mFAILED: %s\033[0m\n' "$1"; exit 1; }

if (( ui )); then
  step "UI: dependencies (lockfile), type-check, build"
  pnpm install --frozen-lockfile --silent || fail "pnpm install"
  pnpm check || fail "UI type-check"
  pnpm build >/dev/null || fail "UI build"
fi

step "macOS: lint"
cargo clippy --workspace --all-targets --locked -- -D warnings || fail "macOS lint"

step "macOS: tests"
cargo test --workspace --locked || fail "macOS tests"

# The Windows target type-checks from here. CARGO_FEATURE_PURE keeps blake3 from needing
# Microsoft's assembler; real Windows builds (CI) use the fast assembly version.
step "Windows: lint"
CARGO_FEATURE_PURE=1 CARGO_TARGET_DIR=target/wincheck \
  cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings || fail "Windows lint"

step "Windows: engine tests"
if ! command -v cargo-xwin >/dev/null; then
  echo "cargo-xwin not installed (cargo install cargo-xwin): skipping; CI will run them."
else
  out=$(CARGO_FEATURE_PURE=1 CARGO_TARGET_DIR=target/xwin \
    cargo xwin test -p pencleaner-core --target x86_64-pc-windows-msvc --no-run --message-format short 2>&1) || { echo "$out"; fail "Windows test build"; }
  exe=$(echo "$out" | sed -n 's/.*Executable unittests src\/lib.rs (\(.*\.exe\)).*/\1/p' | tail -1)
  if command -v wine >/dev/null && [[ -n "$exe" ]]; then
    WINEDEBUG=-all wine "$exe" || fail "Windows tests (under Wine)"
  else
    echo "Built ${exe:-the Windows test program}; install Wine to run it here. CI runs it on real Windows."
  fi
fi

printf '\n\033[32mAll checks passed.\033[0m\n'
