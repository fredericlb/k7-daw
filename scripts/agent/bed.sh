#!/bin/bash
# bed.sh — the one way an agent runs the e2e bed: this Mac's bed slot, once it is free.
# Parameters are e2e-macos.sh's: E2E_SUITE (web, conformance, both), E2E_FILTER, K7_BED_SOUND; the front runs one mode, every piece native.
# The script judges the native counters (bedCounters.mjs); a branch is judged against main's run by sameFailures.mjs.
# --dry-run says what would run and starts nothing; K7_BED_REF names a reference e2e-out.
# Results land in $K7_BED_OUT (default ~/.cache/k7-bed/<worktree>)/mac/e2e-out; E2E_MAX_FAILURES (10) stops a broken run early.
# What a reader gets wrong: the bed runs the working tree, uncommitted edits included (e2e/docs/e2e.md, "The bed dispatcher").
set -euo pipefail

dry=0
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) dry=1 ;;
    *) echo "usage: bed.sh [--dry-run]"; exit 2 ;;
  esac
  shift
done

tree=$(git rev-parse --show-toplevel)
cd "$tree"
suite=${E2E_SUITE:-web}
filter=${E2E_FILTER:-}
base=${K7_BED_OUT:-$HOME/.cache/k7-bed/$(basename "$tree")}
case "$suite" in web | conformance | both) ;; *) echo "E2E_SUITE: web, conformance or both, not '$suite'"; exit 2 ;; esac
[ -z "$(git status --porcelain)" ] || echo "bed.sh: the tree has uncommitted work, which the bed runs"

# A bed slot: its ports silent and no live owner on its lock; slot 1 also no run of an older tree holding it.
slot_free() {
  local o=$(( ($1 - 1) * 100 )) p lock="$HOME/k7-trees/e2e-macos-slot-$1.lock"
  for p in $((28789 + o)) $((28790 + o)) $((25223 + o)) $((28765 + o)); do lsof -nP -iTCP:"$p" -sTCP:LISTEN > /dev/null 2>&1 && return 1; done
  [ "$1" != 1 ] || [ ! -d "$HOME/k7-trees/e2e-macos-bed.lock" ] || return 1
  [ ! -d "$lock" ] || ! kill -0 "$(cat "$lock/pid" 2> /dev/null || echo 0)" 2> /dev/null || return 1
  return 0
}
# Both suites run side by side on slots 1 and 2 (e2e-macos.sh); one suite needs slot 1.
mac_free() { slot_free 1 && { [ "$suite" != both ] || slot_free 2; }; }

# Only one agent claims the bed at a time, so two never see it free together (a lock folder, its owner's pid checked).
dispatch_lock="$HOME/.cache/k7-bed/dispatch.lock"
take_dispatch() {
  mkdir -p "$(dirname "$dispatch_lock")"
  for _ in $(seq 300); do
    if mkdir "$dispatch_lock" 2> /dev/null; then echo $$ > "$dispatch_lock/pid"; return 0; fi
    kill -0 "$(cat "$dispatch_lock/pid" 2> /dev/null || echo 0)" 2> /dev/null || rm -rf "$dispatch_lock"
    sleep 1
  done
  return 1
}
give_dispatch() { [ "$(cat "$dispatch_lock/pid" 2> /dev/null)" = $$ ] && rm -rf "$dispatch_lock"; return 0; }
trap give_dispatch EXIT

# Holds the claim lock once the bed is free; waits up to four hours otherwise.
claim_bed() {
  for _ in $(seq 480); do
    take_dispatch || { echo "bed.sh: the claim lock is held too long" >&2; return 1; }
    mac_free && return 0
    give_dispatch
    # A dry run does not wait: it says it would.
    [ "$dry" = 0 ] || { echo "bed.sh (dry run): the bed is busy: it would wait"; return 0; }
    echo "bed.sh: the bed is busy, waiting" >&2
    sleep 30
  done
  echo "bed.sh: the bed did not come free in four hours" >&2
  return 1
}

claim_bed || exit 1

if [ "$dry" = 1 ]; then
  echo "bed.sh (dry run): suite $suite, filter '$filter'"
  echo "  would run, from $tree: K7_CI_WORKSPACE=$base/mac bash engine/scripts/ci/e2e-macos.sh"
  echo "  Playwright flags: '$(bash engine/scripts/ci/bed-retries.sh)' (empty: no retries, the load is low)"
  exit 0
fi

dest="$base/mac"
echo "=== bed.sh: mac, suite $suite, filter '$filter' ==="
mkdir -p "$dest"
K7_LOOP_SITES=1 K7_CI_WORKSPACE="$dest" STUDIO_FETCHCONTENT_DIR="${STUDIO_FETCHCONTENT_DIR:-$HOME/k7-trees/deps-macos}" \
  bash engine/scripts/ci/e2e-macos.sh &
pid=$!
# The script takes its slot's lock first thing: from then on the claim is visible, and the next agent may decide.
for _ in $(seq 60); do
  [ -d "$HOME/k7-trees/e2e-macos-slot-1.lock" ] && break
  kill -0 "$pid" 2> /dev/null || break
  sleep 1
done
give_dispatch
status=0
wait "$pid" || status=1
echo "bed.sh: results in $base"
exit $status
