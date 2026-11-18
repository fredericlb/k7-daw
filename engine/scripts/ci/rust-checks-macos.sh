#!/bin/bash
# rust-checks-macos.sh <branch> — OneDev's "rust checks macos": clippy and the tests of apps/k7/backend-rs and the browser crates (fmt too) on the macOS agent.
# Each ci/ branch keeps its target folder in ~/k7-ci between runs; one not checked for a week is removed (docs/release.md).
# An area the branch did not touch since base/main is skipped (engine/scripts/ci/areas.mjs, docs/rust-builds.md).
set -euo pipefail
slots="$HOME/k7-ci"
slot="$slots/$(printf %s "$1" | tr -c 'A-Za-z0-9._-' '_')"
mkdir -p "$slot"
find "$slots" -mindepth 1 -maxdepth 1 -type d -mtime +7 -exec rm -rf {} +
touch "$slot"
npm ci --ignore-scripts --no-audit --no-fund
areas="$(node engine/scripts/ci/areas.mjs)"
echo "areas touched since base/main: [$areas]"
# rustc through sccache when the agent has one, on the checks' own server and folder (docs/rust-builds.md).
while IFS= read -r kv; do export "${kv?}"; done < <(node engine/scripts/sccache.mjs checks)
# Five of the agent's cores: the engine and tray builds share the machine (docs/release.md).
# --target-dir, not CARGO_TARGET_DIR: sccache keys on every CARGO_ variable, and the folder is per branch.
target="$slot/target"
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=5 NEXTEST_TEST_THREADS=5
root="$PWD"
# The current folder's tests, every binary run to the end; nextest runs them all at once, and these crates hold no doctest it would miss.
tests() {
  if cargo nextest --version > /dev/null 2>&1; then
    cargo nextest run --locked --target-dir "$target" --no-fail-fast "$@"
  else
    echo "no cargo-nextest on this agent: cargo test instead"
    cargo test --locked --target-dir "$target" --no-fail-fast "$@"
  fi
}
if [[ " $areas " == *" backend "* ]]; then
  (cd apps/k7/backend-rs && cargo clippy --locked --target-dir "$target" --workspace --all-targets -- -D warnings && tests --workspace)
fi
if [[ " $areas " == *" browser "* ]]; then
  # The browser's crates, each its own Rust workspace (packages/song-schema/docs/wasmBoundary.md); rustfmt is not in every rustup profile.
  rustup component add rustfmt > /dev/null 2>&1 || true
  for crate in packages/song-schema/rust packages/k7-wasm/rust packages/song/rust; do
    (cd "$root/$crate" && cargo fmt --check && cargo clippy --locked --target-dir "$target" --all-targets --features ts -- -D warnings && tests)
  done
  # The song crate's browser build is not optional: the target is added here, not looked for.
  rustup target add wasm32-unknown-unknown
  (cd "$root/packages/song/rust" && cargo clippy --locked --target wasm32-unknown-unknown --no-default-features --target-dir "$target" -- -D warnings && cargo build --locked --target wasm32-unknown-unknown --no-default-features --target-dir "$target")
  if rustup target list --installed 2> /dev/null | grep -q wasm32-unknown-unknown; then
    (cd "$root/packages/k7-wasm/rust" && cargo build --locked --release --target wasm32-unknown-unknown --target-dir "$target")
  fi
fi
if [ -n "${RUSTC_WRAPPER:-}" ]; then "$RUSTC_WRAPPER" --show-stats; fi
