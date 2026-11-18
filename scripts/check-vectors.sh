#!/usr/bin/env bash
# check-vectors.sh — regenerates the k7x editor's and the host's test vectors and fails when the committed ones differ, then replays the frozen file-service vectors; part of `npm run ci:local`.
# What a reader gets wrong: the file service's vectors have no generator (frozen, replayed only); the generators run with an absolute path, the bare file name fails.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
# A merge in progress stages its vectors, which the restore below would discard (docs/lints.md).
if git rev-parse -q --verify MERGE_HEAD > /dev/null; then
  echo "vectors: a merge is in progress; commit it, then run this check" >&2
  exit 1
fi
k7x=packages/k7x/rust/tests/vectors/
npx tsx "$PWD/packages/k7x/scripts/makeEditVectors.mts" > /dev/null
drift="$(git status --porcelain -- "$k7x")"
if [ -n "$drift" ]; then
  echo "vectors: the committed k7x vectors differ from what makeEditVectors.mts writes:" >&2
  echo "$drift" >&2
  git checkout HEAD -- "$k7x"
  git clean -fdq -- "$k7x"
  echo "vectors: restored; commit the regenerated files (docs/lints.md)" >&2
  exit 1
fi
host=apps/k7/backend-rs/crates/k7-mediatek/tests/vectors/
(cd packages/commands && npx tsx "$PWD/scripts/makeHostVectors.mts" > /dev/null)
drift="$(git status --porcelain -- "$host")"
if [ -n "$drift" ]; then
  echo "vectors: the committed host vectors differ from what makeHostVectors.mts writes:" >&2
  echo "$drift" >&2
  git checkout HEAD -- "$host"
  git clean -fdq -- "$host"
  echo "vectors: restored; commit the regenerated files (docs/lints.md)" >&2
  exit 1
fi
# The file service's vectors (tests/vectors/*.json) and song-edit's vectors and goldens are frozen (no generator): they are only replayed, through the crate and through the WebAssembly module.
# The file service's frozen vectors replayed by the crate; a vector edited by hand or a replay broken fails here.
eval "$(node engine/scripts/sccache.mjs agent --export)"
(cd apps/k7/backend-rs && cargo test --locked --target-dir "${K7_VECTORS_TARGET:-$HOME/.cache/k7-vectors}" -p k7-file-service --test all -- vectors rig::)
# The frozen vectors replayed through the WebAssembly module, over a Vfs and a temp folder.
npm run --silent test:edit-replay -w @studio/k7-wasm -- ${K7_EDIT_TARGET:+--target-dir "$K7_EDIT_TARGET"}
bash scripts/agent/proof.sh put vectors
echo "vectors: committed vectors match the generator"
