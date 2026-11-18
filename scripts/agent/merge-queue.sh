#!/usr/bin/env bash
# merge-queue.sh <branch>... — merges the branches in order, one at a time, through merge-branch.sh, stopping at the first that does not merge.
# Takes `.claude/state/merge-queue.lock` (a dead holder's lock is taken over), refreshes slot-6 to main between merges, prints one line per branch.
# What a reader gets wrong: a branch after the failed one is not tried, since it may need the failed one's changes; it is listed as `not run`.
# K7_MERGE_BRANCH overrides merge-branch.sh, K7_QUEUE_SLOT the slot refreshed, K7_QUEUE_WAIT the lock poll seconds (tests).
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
common="$(git -C "$here" rev-parse --path-format=absolute --git-common-dir)"
clone="${common%/.git}"
[ $# -gt 0 ] || { echo "usage: merge-queue.sh <branch>..." >&2; exit 1; }
mb="${K7_MERGE_BRANCH:-bash $here/merge-branch.sh}"
state="$clone/.claude/state"
lock="$state/merge-queue.lock"
mkdir -p "$state"
until mkdir "$lock" 2> /dev/null; do
  holder="$(cat "$lock/pid" 2> /dev/null || true)"
  if [ -z "$holder" ] || ! kill -0 "$holder" 2> /dev/null; then rm -rf "$lock"; continue; fi
  echo "merge-queue: pid $holder is merging; waiting"
  sleep "${K7_QUEUE_WAIT:-30}"
done
echo $$ > "$lock/pid"
trap 'rm -rf "$lock"' EXIT
slot="$clone/.claude/worktrees/slot-${K7_QUEUE_SLOT:-6}"
log="$(mktemp)"
failed=""
summary=()
for br in "$@"; do
  if [ -n "$failed" ]; then summary+=("merge-queue: $br not run"); continue; fi
  if [ -d "$slot" ]; then git -C "$slot" checkout -q --detach main || echo "merge-queue: could not refresh $(basename "$slot") to main" >&2; fi
  # shellcheck disable=SC2086 # the command is a word list
  if $mb "$br" 2>&1 | tee "$log"; then
    summary+=("merge-queue: $br merged")
  else
    # The reason is the last message line that is not the result line itself.
    why="$(grep '^merge-branch:' "$log" | grep -v "^merge-branch: $br \(stopped\|conflicts\)$" | tail -1 | sed 's/^merge-branch: //' || true)"
    kind="$(grep -o "^merge-branch: $br \(stopped\|conflicts\)$" "$log" | tail -1 | awk '{print $3}' || true)"
    summary+=("merge-queue: $br ${kind:-stopped}: ${why:-no reason printed}")
    failed="$br"
  fi
done
rm -f "$log"
printf '%s\n' "${summary[@]}"
[ -z "$failed" ] || { echo "merge-queue: stopped at $failed"; exit 1; }
