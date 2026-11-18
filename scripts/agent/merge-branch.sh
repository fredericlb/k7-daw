#!/usr/bin/env bash
# merge-branch.sh [--dry-run] <branch> [summary] — merges a finished branch into local main by plumbing (docs/agent-playbook.md "Merging").
# Run from outside any slot; it finds the repo from its own location, as slot.sh does. Exit 1 on the first failure.
# Steps: main into the branch (in its slot), gate.sh --quick, the rev-list test, commit-tree, update-ref, the ancestor check, free, delete.
# What a reader gets wrong: main is moved with an expected old value, so a main that moved during the run fails the merge, never loses a commit.
# A branch no slot holds takes main through merge-tree and is not gated. K7_GATE overrides the gate command (tests).
# A branch that changed TypeScript also passes `npm run check:changed` after the gate; K7_CHECK overrides that command (tests).
# A branch that touched a vector's source or the vectors also passes `check-vectors.sh` after that; K7_VECTORS overrides it (tests).
# When it fails, `regen-vectors.sh` regenerates and replays in Rust; passing replays commit the vectors, failing ones refuse (K7_GENERATE, K7_REPLAYS).
# Generated files (`merge=rebuilt` in .gitattributes) are settled here: K7_REBUILD overrides `npm run rebuild:generated`, K7_GENCHECK the staleness check (tests).
# `--dry-run` stops before main moves and prints `merge-branch: <branch> ready|stopped|conflicts`; a step whose proof stamp (proof.sh) names the merged tree is skipped.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
common="$(git -C "$here" rev-parse --path-format=absolute --git-common-dir)"
clone="${common%/.git}"
dry=0
if [ "${1:-}" = "--dry-run" ]; then dry=1; shift; fi
br="${1:-}"
[ -n "$br" ] || { echo "usage: merge-branch.sh [--dry-run] <branch> [summary]" >&2; exit 1; }
die() { echo "merge-branch: $*" >&2; echo "merge-branch: $br stopped"; exit 1; }
# A step passed on this exact tree, in this slot or an earlier run (proof.sh); the slot's tree is HEAD's.
proved() { (cd "$wt" && bash "$here/proof.sh" has "$1"); }
prove() { (cd "$wt" && bash "$here/proof.sh" put "$1"); }
g() { git -C "$clone" "$@"; }
g show-ref --verify -q "refs/heads/$br" || die "no branch $br"
[ "$br" != main ] || die "refusing to merge main"
summary="${2:-$(g log -1 --format=%s "$br" | sed 's/^k7: //')}"
# The worktree holding the branch, if any.
wt="$(g worktree list --porcelain | awk -v b="refs/heads/$br" '/^worktree /{w=substr($0,10)} $1=="branch" && $2==b {print w}')"
slotn=""
if [ -n "$wt" ]; then
  case "$wt" in "$clone"/.claude/worktrees/slot-*) slotn="${wt##*/slot-}" ;; esac
  [ -n "$slotn" ] || die "$br is checked out in $wt, which is not a slot"
  dirty="$(git -C "$wt" status --porcelain)"
  [ -z "$dirty" ] || die "slot-$slotn has uncommitted files: $(echo "$dirty" | tr '\n' ' ')"
fi
if [ -n "$wt" ]; then
  rebuild="${K7_REBUILD:-npm run rebuild:generated}"
  # A path is generated when .gitattributes says merge=rebuilt; the list is never hard-coded.
  is_rebuilt() { [ "$(git -C "$wt" check-attr merge -- "$1" | sed 's/^.*: //')" = rebuilt ]; }
  # The paths with changes against HEAD, tracked or not.
  changed_paths() { git -C "$wt" status --porcelain -uall | sed 's/^...//'; }
  built=0
  head_before="$(git -C "$wt" rev-parse HEAD)"
  if ! git -C "$wt" merge --no-edit main > /dev/null; then
    conflicts="$(git -C "$wt" diff --name-only --diff-filter=U)"
    only_rebuilt=1
    for f in $conflicts; do is_rebuilt "$f" || only_rebuilt=0; done
    if [ -z "$conflicts" ] || [ "$only_rebuilt" = 0 ]; then
      echo "merge-branch: conflicts merging main into $br:" >&2
      echo "$conflicts" >&2
      git -C "$wt" merge --abort
      echo "merge-branch: $br conflicts"
      exit 1
    fi
    echo "merge-branch: only generated files conflict, rebuilding: $(echo "$conflicts" | tr '\n' ' ')"
    for f in $conflicts; do git -C "$wt" checkout --ours -- "$f" 2> /dev/null || true; done
    if ! (cd "$wt" && eval "$rebuild"); then
      git -C "$wt" merge --abort
      die "rebuild:generated failed in slot-$slotn: merge aborted"
    fi
    for f in $conflicts; do git -C "$wt" add -A -- "$f"; done
    built=1
    git -C "$wt" commit --no-edit -q || die "could not commit the merge in slot-$slotn"
  fi
  # A merge of main done by hand leaves HEAD unmoved, so staleness is asked of generated-check too.
  genstale=0
  if [ "$(git -C "$wt" rev-parse HEAD)" = "$head_before" ] && ! (cd "$wt" && eval "${K7_GENCHECK:-node scripts/agent/generated-check.mjs}") > /dev/null 2>&1; then genstale=1; fi
  if [ "$(git -C "$wt" rev-parse HEAD)" != "$head_before" ] || [ "$genstale" = 1 ]; then
    echo "merge-branch: $br has main merged or stale generated files: rebuild:generated"
    [ "$built" = 1 ] || (cd "$wt" && eval "$rebuild") || die "rebuild:generated failed in slot-$slotn"
    stale="$(changed_paths)"
    if [ -n "$stale" ]; then
      foreign=""
      for f in $stale; do is_rebuilt "$f" || foreign="$foreign $f"; done
      [ -z "$foreign" ] || die "rebuild:generated changed files that are not merge=rebuilt, left in slot-$slotn:$foreign"
      for f in $stale; do git -C "$wt" add -A -- "$f"; done
      git -C "$wt" commit -q -m "k7: generated files rebuilt after merging main

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" || die "could not commit the rebuilt files in slot-$slotn"
    fi
  fi
  gate="${K7_GATE:-bash $wt/scripts/agent/gate.sh --quick}"
  if proved gate-quick; then echo "merge-branch: reused the quick gate's proof for this tree"
  else (cd "$wt" && eval "$gate") || die "the quick gate failed in slot-$slotn"; prove gate-quick; fi
  # The branch's own changes against main, taken after main is merged in.
  if git -C "$wt" diff --name-only main...HEAD | grep -Eq '\.(ts|tsx|mts|mjs)$'; then
    echo "merge-branch: $br touches TypeScript: check:changed"
    if proved check; then echo "merge-branch: reused check:changed's proof for this tree"
    else (. "$here/gate-lib.sh"; gate_timeout_scale; cd "$wt" && eval "${K7_CHECK:-npm run check:changed}") || die "check:changed failed in slot-$slotn"; prove check; fi
  fi
  vec_re='^(packages/.*\.m?ts$|demos/|k7-core/|apps/k7/backend-rs/(crates/k7-file-service/)?tests/vectors/|packages/[^/]+/rust/tests/(vectors|goldens)/)'
  if git -C "$wt" diff --name-only main...HEAD | grep -Eq "$vec_re"; then
    echo "merge-branch: $br touches vector sources or vectors: check-vectors"
    if proved vectors; then echo "merge-branch: reused check-vectors' proof for this tree"
    elif (cd "$wt" && eval "${K7_VECTORS:-bash scripts/check-vectors.sh}"); then prove vectors
    else
      # Drift after merging main is regenerated and settled when the Rust replays pass (regen-vectors.sh).
      echo "merge-branch: vectors drifted: regen-vectors"
      (cd "$wt" && bash scripts/agent/regen-vectors.sh "$slotn") || die "regen-vectors failed in slot-$slotn: the vectors are restored, see above for the failing step"
      regen="$(changed_paths | grep -E 'tests/(vectors|goldens)/' || true)"
      if [ -n "$regen" ]; then
        for f in $regen; do git -C "$wt" add -A -- "$f"; done
        git -C "$wt" commit -q -m "k7: vectors regenerated after merging main

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" || die "could not commit the regenerated vectors in slot-$slotn"
      fi
    fi
  fi
else
  echo "merge-branch: no slot holds $br: main taken by merge-tree, no gate"
  tree="$(g merge-tree --write-tree main "$br")" || { echo "merge-branch: conflicts merging main into $br" >&2; echo "merge-branch: $br conflicts"; exit 1; }
  merged="$(g commit-tree "$tree" -p "$br" -p main -m "Merge branch 'main' into $br")"
  g update-ref "refs/heads/$br" "$merged" "$(g rev-parse "$br")"
fi
if [ "$dry" = 1 ]; then echo "merge-branch: $br ready"; exit 0; fi
old="$(g rev-parse main)"
msg="Merge branch '$br': $summary

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
test "$(g rev-list --count "$br..main")" = 0 \
  && new="$(g commit-tree "$br^{tree}" -p main -p "$br" -m "$msg")" \
  && g update-ref refs/heads/main "$new" "$old" \
  && g merge-base --is-ancestor "$br" main \
  && { [ -z "$slotn" ] || bash "$here/slot.sh" free "$slotn"; } \
  && g branch -D "$br" > /dev/null \
  && mkdir -p "$clone/.claude/state" \
  && echo "$(date '+%Y-%m-%d %H:%M') $br $new" >> "$clone/.claude/state/merges.log" \
  && echo "merge-branch: $br merged into main at $new" \
  || die "stopped: main moved since the gate, or a step failed (main is at $(g rev-parse main)); rerun once the branch has taken main"
