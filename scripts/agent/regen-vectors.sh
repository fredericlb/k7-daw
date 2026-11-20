#!/usr/bin/env bash
# regen-vectors.sh <slot> — regenerates the test vectors check-vectors.sh regenerates (the k7x editor's), then runs the Rust replay that reads them.
# Exit 0: regenerated and every replay passed (files left changed, not committed). Exit 1: a generator or replay failed, files restored, the failing step named.
# What a reader gets wrong: the generators run with the same cwd and absolute paths as check-vectors.sh. K7_GENERATE and K7_REPLAYS override the two stages (tests).
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
slot="${1:-0}"
cd "$root"
paths=(apps/k7-daw-wasm/k7x/tests/vectors/)
restore() { git checkout HEAD -- "${paths[@]}"; git clean -fdq -- "${paths[@]}"; }
fail() { echo "regen-vectors: $* failed; vectors restored" >&2; restore; exit 1; }
generate() {
  npx tsx "$PWD/apps/k7-daw-frontend/packages/k7x/scripts/makeEditVectors.mts" > /dev/null || return 1
}
if [ -n "${K7_GENERATE:-}" ]; then eval "$K7_GENERATE" || fail "the generators"; else generate || fail "the generators"; fi
if [ -n "${K7_REPLAYS:-}" ]; then eval "$K7_REPLAYS" || fail "a replay"; exit 0; fi
eval "$(node apps/k7-daw-engine/scripts/sccache.mjs agent --export)"
td="$HOME/.cache/k7-$slot"
run() { local name="$1"; shift; echo "regen-vectors: replay $name"; bash scripts/agent/throttle.sh "$@" || fail "replay $name"; }
run "song-edit cargo test" cargo test --manifest-path apps/k7-daw-wasm/song-edit/Cargo.toml --target-dir "$td" --
run "k7x edit_vectors" cargo test --manifest-path apps/k7-daw-wasm/k7x/Cargo.toml --target-dir "$td" --test edit_vectors --
