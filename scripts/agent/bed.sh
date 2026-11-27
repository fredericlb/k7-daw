#!/bin/bash
# bed.sh — the one way an agent runs the e2e bed: e2e-macos.sh in this worktree, which waits for a free bed slot itself.
# Parameters are e2e-macos.sh's: E2E_SUITE (web, conformance, both), E2E_FILTER, K7_BED_SOUND; the front runs one mode, every piece native.
# The script judges the native counters (bedCounters.mjs); a branch is judged against main's run by sameFailures.mjs.
# --dry-run says what would run and starts nothing; K7_BED_REF names a reference e2e-out.
# Results land in $K7_BED_OUT (default ~/.cache/k7-bed/<worktree>)/mac/e2e-out; E2E_MAX_FAILURES (10) stops a broken run early.
# What a reader gets wrong: the bed runs the working tree, uncommitted edits included (e2e/docs/e2e.md, "The bed dispatcher").
set -euo pipefail

dry=0
if [ "${1:-}" = --dry-run ]; then dry=1; shift; fi
[ $# -eq 0 ] || { echo "usage: bed.sh [--dry-run]"; exit 2; }

tree=$(git rev-parse --show-toplevel)
cd "$tree"
# A worktree whose @studio/* resolve into the main checkout (no node_modules, or one linked to main's) gets files the bed's Vite refuses.
wasm_pkg="$(realpath "$tree/node_modules/@studio/k7-wasm" 2>/dev/null || true)"
if [ "$(git rev-parse --git-common-dir)" != "$(git rev-parse --git-dir)" ] && [ "${wasm_pkg#"$tree"/}" = "$wasm_pkg" ]; then
  echo "bed.sh: @studio/* resolve outside this worktree (${wasm_pkg:-nowhere}); relinking (worktree-init, no install)"
  [ -L "$tree/node_modules" ] && rm "$tree/node_modules"
  node scripts/agent/worktree-init.mjs
fi
suite=${E2E_SUITE:-web}
dest=${K7_BED_OUT:-$HOME/.cache/k7-bed/$(basename "$tree")}/mac
case "$suite" in web | conformance | both) ;; *) echo "E2E_SUITE: web, conformance or both, not '$suite'"; exit 2 ;; esac
[ -z "$(git status --porcelain)" ] || echo "bed.sh: the tree has uncommitted work, which the bed runs"

if [ "$dry" = 1 ]; then
  echo "bed.sh (dry run): suite $suite, filter '${E2E_FILTER:-}'"
  echo "  would run, from $tree: K7_CI_WORKSPACE=$dest bash apps/k7-daw-engine/scripts/ci/e2e-macos.sh"
  echo "  Playwright flags: '$(bash apps/k7-daw-engine/scripts/ci/bed-retries.sh)' (empty: no retries, the load is low)"
  exit 0
fi

echo "=== bed.sh: mac, suite $suite, filter '${E2E_FILTER:-}' ==="
mkdir -p "$dest"
status=0
K7_CI_WORKSPACE="$dest" STUDIO_FETCHCONTENT_DIR="${STUDIO_FETCHCONTENT_DIR:-$HOME/k7-trees/deps-macos}" \
  bash apps/k7-daw-engine/scripts/ci/e2e-macos.sh || status=1
echo "bed.sh: results in $dest"
exit $status
