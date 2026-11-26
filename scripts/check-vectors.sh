#!/usr/bin/env bash
# check-vectors.sh — replays the frozen vectors no generator writes any more: k7x's editor, the file service's, song-edit's; part of `npm run ci:local`.
# What a reader gets wrong: nothing is regenerated here; a vector that fails is a change of the Rust it replays, never a file to rewrite.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
eval "$(node apps/k7-daw-engine/scripts/sccache.mjs agent --export)"
# The k7x editor's vectors, which the TypeScript editor wrote before it left (apps/k7-daw-wasm/k7x/docs/rustEdit.md).
(cd apps/k7-daw-wasm/k7x && cargo test --locked --target-dir "${K7_VECTORS_TARGET:-$HOME/.cache/k7-vectors}" --test edit_vectors --test corpus)
# The file service's frozen vectors replayed by the crate; a vector edited by hand or a replay broken fails here.
(cd apps/k7-daw-backend && cargo test --locked --target-dir "${K7_VECTORS_TARGET:-$HOME/.cache/k7-vectors}" -p k7-file-service --test all -- vectors rig::)
# song-edit's frozen vectors and goldens replayed by the crate itself.
(cd apps/k7-daw-wasm/song-edit && cargo test --locked --release --target-dir "${K7_EDIT_TARGET:-$HOME/.cache/k7-edit-replay}" --test replay --test replay_lane)
echo "vectors: the frozen vectors replay"
