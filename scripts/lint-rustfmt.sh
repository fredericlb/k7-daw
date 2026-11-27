#!/usr/bin/env bash
# lint-rustfmt.sh — every Rust workspace of the repo formatted as rustfmt prints it, checked whole (docs/lints.md).
# What a reader gets wrong: gate.sh checks only the files a branch changed, so this is what keeps the rest from drifting.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
status=0
for dir in apps/k7-daw-backend apps/k7-daw-desktop/src-tauri apps/k7-daw-wasm/song apps/k7-daw-wasm/song-schema apps/k7-daw-wasm/k7-wasm apps/k7-daw-wasm/k7x apps/k7-daw-wasm/git-rules; do
  (cd "$dir" && cargo fmt --all --check > /dev/null) || { echo "lint:rustfmt: $dir is not formatted: run cargo fmt --all there"; status=1; }
done
[ "$status" = 0 ] && echo "lint:rustfmt: every workspace formatted"
exit "$status"
