#!/usr/bin/env bash
# lint-rustfmt.sh — every Rust workspace of the repo formatted as rustfmt prints it, checked whole (docs/lints.md).
# What a reader gets wrong: gate.sh checks only the files a branch changed, so this is what keeps the rest from drifting.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
status=0
for dir in apps/k7/backend-rs engine/k7-supervisor/rust apps/k7-desktop/src-tauri packages/song/rust packages/song-schema/rust packages/k7-wasm/rust packages/k7x/rust; do
  (cd "$dir" && cargo fmt --all --check > /dev/null) || { echo "lint:rustfmt: $dir is not formatted: run cargo fmt --all there"; status=1; }
done
[ "$status" = 0 ] && echo "lint:rustfmt: every workspace formatted"
exit "$status"
