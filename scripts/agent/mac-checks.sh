#!/usr/bin/env bash
# mac-checks.sh — the Rust checks a branch passes on the Mac before it merges: clippy and the tests of the backend and the browser crates, and the song crate's wasm build.
# It checks the worktree it is run from, skipping an area the branch did not touch since main (apps/k7-daw-engine/scripts/ci/areas.mjs); `--all` runs both.
# `--narrow` checks only the backend crates touched since K7_CRATES_BASE and those depending on them (scripts/agent/affected-crates.mjs).
# `--narrow` also filters the slow twins and churn tests by piece (scripts/agent/affected-tests.mjs; K7_TESTS=all runs them all).
# `--clippy` (with `--narrow`) stops after clippy; a failed test is rerun alone and a load flake is carried past (scripts/agent/rerun-failed.sh).
# What a reader gets wrong: CI does not run these for a branch any more (docs/agent-playbook.md), so a skipped check here is skipped everywhere.
set -euo pipefail
# shellcheck source=throttle.sh
. "$(dirname "$0")/throttle.sh" "$@"
root="$(git rev-parse --show-toplevel)"
cd "$root"
# The worktree's one build folder, the agent's own: a check reuses what the agent already compiled (docs/agent-playbook.md).
target="${K7_TARGET_DIR:-$HOME/.cache/k7-$(basename "$root")}"
eval "$(node apps/k7-daw-engine/scripts/sccache.mjs agent --export)"
# shellcheck source=rerun-failed.sh
source "$root/scripts/agent/rerun-failed.sh"
clippy_only=""
[ "${2:-}" = "--clippy" ] && clippy_only=1
if [ "${1:-}" = "--all" ]; then areas="backend browser"; else areas="$(K7_AREAS_BASE="${K7_GATE_BASE:-main}" node apps/k7-daw-engine/scripts/ci/areas.mjs)"; fi
crates="--workspace"
# Under --narrow a piece's twins and churn tests run only when that piece changed (affected-tests.mjs); the merge gate runs them all.
slow_filter=""
if [ "${1:-}" = "--narrow" ]; then
  crates="$(node scripts/agent/affected-crates.mjs)"
  slow_filter="$(node scripts/agent/affected-tests.mjs)"
fi
[ -n "$crates" ] || areas="${areas/backend/}"
echo "mac-checks: areas [$areas], backend crates [$crates], target $target"
tests() {
  [ -z "$clippy_only" ] || return 0
  if cargo nextest --version > /dev/null 2>&1; then
    # Only the backend call names crates; the filter is the first run's, a rerun names its own tests.
    K7_NEXTEST_FILTER="$([ $# -eq 0 ] || printf '%s' "$slow_filter")" rerun_nextest --locked --target-dir "$target" --no-fail-fast "$@"
  else
    cargo test --locked --target-dir "$target" --no-fail-fast "$@"
  fi
}
if [[ " $areas " == *" backend "* ]]; then
  # shellcheck disable=SC2086 # one word per -p flag
  (cd apps/k7-daw-backend && cargo clippy --locked --target-dir "$target" $crates --all-targets -- -D warnings && tests $crates)
fi
if [[ " $areas " == *" browser "* ]]; then
  for crate in apps/k7-daw-wasm/song-schema apps/k7-daw-wasm/gesture apps/k7-daw-wasm/git-rules apps/k7-daw-wasm/k7-wasm apps/k7-daw-wasm/song; do
    (cd "$crate" && cargo fmt --check && cargo clippy --locked --target-dir "$target" --all-targets --features ts -- -D warnings && tests)
  done
  (cd apps/k7-daw-wasm/k7x && cargo fmt --check && cargo clippy --locked --target-dir "$target" --all-targets -- -D warnings && tests)
  (cd apps/k7-daw-wasm/k7x && cargo clippy --locked --target wasm32-unknown-unknown --lib --target-dir "$target" -- -D warnings)
  (cd apps/k7-daw-wasm/song-edit && cargo fmt --check && cargo clippy --locked --target-dir "$target" --all-targets -- -D warnings && tests)
  (cd apps/k7-daw-wasm/song-edit && cargo clippy --locked --target wasm32-unknown-unknown --lib --target-dir "$target" -- -D warnings && { [ -n "$clippy_only" ] || cargo build --locked --target wasm32-unknown-unknown --lib --target-dir "$target"; })
  node apps/k7-daw-wasm/verbs/scripts/catalogue.mjs --check
  (cd apps/k7-daw-wasm/k7-wasm && tests --features verbs)
  (cd apps/k7-daw-wasm/verbs && cargo fmt --check && cargo clippy --locked --target-dir "$target" --all-targets -- -D warnings && tests)
  (cd apps/k7-daw-wasm/song && cargo clippy --locked --target wasm32-unknown-unknown --no-default-features --target-dir "$target" -- -D warnings && { [ -n "$clippy_only" ] || cargo build --locked --target wasm32-unknown-unknown --no-default-features --target-dir "$target"; })
fi
echo "mac-checks: passed"
