#!/usr/bin/env bash
# stage-gates.sh — the song crate against the TypeScript over the golden corpus, its sweeps and the twins, then the golden smoke; part of `npm run ci:local`.
# They hold on any platform, so they run on the Mac, not in a CI job (docs/rust-builds.md).
# What a reader gets wrong: nothing changed since the merge-base with main is skipped, and on main since the last main they passed
# on (`--all` runs anyway); the TypeScript's outputs are cached by a hash of what they read (`--fresh` ignores it).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
reads=(packages/song packages/song-schema packages/k7x packages/dsp packages/graph-model
  packages/engine-protocol packages/web-protocol apps/k7/backend-rs/crates/k7-file-service/tests/all/golden_smoke.rs k7-core demos e2e/fixtures
  package-lock.json scripts/stage-gates.sh)
all=0 fresh=0 keyonly=0
for a in "$@"; do case "$a" in --all) all=1 ;; --fresh) fresh=1 ;; --key) all=1 keyonly=1 ;; *) echo "usage: stage-gates.sh [--all] [--fresh] [--key]" >&2; exit 2 ;; esac; done
# On main itself the merge-base is HEAD, so the last main that passed is the base, and with none recorded everything runs.
passed_on_main="$HOME/.cache/k7-stage-gates-main"
on_main=0
[ "$(git rev-parse HEAD)" = "$(git rev-parse main)" ] && on_main=1
if [ "$all" = 0 ] && [ "$on_main" = 1 ] && ! git cat-file -e "$(cat "$passed_on_main" 2> /dev/null || echo none)^{commit}" 2> /dev/null; then
  all=1
fi
if [ "$all" = 0 ]; then
  base="$(git merge-base HEAD main)"
  [ "$on_main" = 1 ] && base="$(cat "$passed_on_main")"
  if git diff --quiet "$base" -- "${reads[@]}" && git diff --quiet HEAD -- "${reads[@]}"; then
    echo "stage gates: nothing they read changed since $(git rev-parse --short "$base"), skipped (--all runs them)"
    exit 0
  fi
fi
eval "$(node engine/scripts/sccache.mjs dev --export)"
target="$HOME/.cache/k7-stage-gates"
cache="$HOME/.cache/k7-stage-dumps"
stage="$(cd "$(mktemp -d "${TMPDIR:-/tmp}/k7-song-stage.XXXXXX")" && pwd -P)"
trap 'rm -rf "$stage"' EXIT
song() { (cd packages/song && npx tsx "$@"); }
# gate <module> [cargo flags]: the module of the song crate's one test binary (tests/all) whose ignored tests run.
gate() { local m="$1"; shift; (cd packages/song/rust && cargo test --locked --target-dir "$target" "$@" --test all -- --ignored --nocapture "$m::"); }
# The sweeps' sizes and seeds are part of the cache key: the floors the gates hold depend on them.
views_args="--sweep-clips 1500 --sweep-wav 2000 --seed 0x2d4a0e77"
graph_args="--sweep 1500 --seed 0x2c0f1a5b"
# Everything the TypeScript half reads; the Rust sources are left out, the JSON cases the dump scripts read are not.
ts_reads=(packages k7-core demos e2e/fixtures package-lock.json
  ':(exclude,glob)packages/*/rust/**/*.rs' ':(exclude,glob)packages/*/rust/**/Cargo.*' ':(exclude,glob)packages/*/rust/**/rustfmt.toml'
  ':(exclude,glob)packages/*/rust/**/*.snap' ':(exclude,glob)packages/*/docs/**')
cache_key() {
  { echo "$views_args|$graph_args"; git ls-files -s -- "${ts_reads[@]}"; git diff HEAD --binary -- "${ts_reads[@]}"
    git ls-files -o --exclude-standard -z -- "${ts_reads[@]}" | xargs -0 shasum 2>/dev/null; } | shasum -a 256 | cut -c1-64
}
clone() { cp -cR "$1" "$2" 2>/dev/null || cp -R "$1" "$2"; }
# retarget <dir> <old> <new>: a staged tree and its dumps spell the folder they were staged in.
retarget() { grep -rlF --null -- "$2" "$1" | O="$2" N="$3" xargs -0 -n 64 -P 4 perl -pi -e 's/\Q$ENV{O}\E/$ENV{N}/g' || true; }
tmp_of() { node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).tmp)' "$1/manifest.json"; }
# unstage <name>: the cached tree `name` cloned into this run, spelling this run's folders and checkout.
unstage() {
  local old_stage old_root
  old_stage="$(dirname "$(dirname "$(tmp_of "$entry/staged")")")"; old_root="$(cat "$entry/root")"
  clone "$entry/$1" "$stage/$1"
  retarget "$stage/$1" "$old_stage" "$stage"
  [ "$old_root" = "$root" ] && return 0
  retarget "$stage/$1" "$old_root" "$root"
  # A link into the checkout the entry was made in would dangle once that worktree goes.
  find "$stage/$1" -type l -lname "$old_root/*" | while read -r l; do t="$(readlink "$l")"; ln -sfn "$root/${t#"$old_root"/}" "$l"; done
}
produce() {
  song scripts/stageCorpus.ts "$stage/staged"
  song scripts/stageDump.ts "$stage/staged" "$stage/dumps"
  song scripts/curveDump.ts "$stage/staged" "$stage/dumps"
  song scripts/stageDumpClips.ts "$stage/staged" "$stage/dumps/clips"
  song scripts/stageDumpLanes.ts "$stage/staged" "$stage/dumps/lanes"
  song scripts/stageDumpPictures.ts "$stage/staged" "$stage/dumps/pictures"
  # shellcheck disable=SC2086
  song scripts/stageDumpViews.ts "$stage/staged" "$stage/dumps/views" $views_args
  # shellcheck disable=SC2086
  song scripts/stageDumpGraph.ts "$stage/staged" "$stage/dumps/graph" $graph_args
  # The corpus as a smoke test: every project compiles natively; a stage serves one run.
  song scripts/stageCorpus.ts "$stage/smoke"
}
key="$(cache_key)"
[ "$keyonly" = 1 ] && { echo "$key"; exit 0; }
entry="$cache/$key"
if [ "$fresh" = 0 ] && [ -f "$entry/complete" ]; then
  unstage staged; unstage dumps; unstage smoke; touch "$entry"
  echo "stage gates: reused the staged corpus, the dumps and the smoke stage (cache ${key:0:12})"
else
  produce
  # Stored before any gate runs, since a compile repairs the tree it reads.
  mkdir -p "$cache"; part="$(mktemp -d "$cache/.part.XXXXXX")"
  for d in staged dumps smoke; do clone "$stage/$d" "$part/$d"; done
  echo "$root" > "$part/root"; touch "$part/complete"; rm -rf "$entry"; mv "$part" "$entry"
  # shellcheck disable=SC2012
  ls -t "$cache" | tail -n +4 | while read -r old; do rm -rf "${cache:?}/$old"; done
  echo "stage gates: computed the TypeScript outputs and cached them (cache ${key:0:12})"
fi
K7_SONG_STAGED="$stage/staged" K7_SONG_DUMPS="$stage/dumps" gate stages
K7_SONG_STAGED="$stage/staged" K7_SONG_DUMPS="$stage/dumps" gate curves_stage
K7_SONG_STAGED="$stage/staged" K7_SONG_DUMPS="$stage/dumps/clips" gate stages_clips
K7_SONG_STAGED="$stage/staged" K7_SONG_DUMPS="$stage/dumps/lanes" gate stages_lanes
K7_SONG_STAGED="$stage/staged" K7_SONG_DUMPS="$stage/dumps/pictures" gate stages_pictures
K7_SONG_STAGED="$stage/staged" K7_SONG_DUMPS="$stage/dumps/views" gate stages_views
K7_SONG_STAGED="$stage/staged" K7_SONG_DUMPS="$stage/dumps/graph" gate stages_graph
# The twins are written by the Rust and checked by the TypeScript, so they are never cached.
K7_SONG_TWIN="$stage/twin-views" gate views_twin --features ts
song scripts/viewsTwinCheck.ts "$stage/twin-views"
K7_SONG_TWIN="$stage/twin-graph" gate graph_twin --features ts
song scripts/graphTwinCheck.ts "$stage/twin-graph"
(cd apps/k7/backend-rs && K7_GOLDEN_STAGED="$stage/smoke" cargo test --locked --target-dir "$target" -p k7-file-service --test all -- --ignored golden_smoke::)
[ "$on_main" = 0 ] || git rev-parse HEAD > "$passed_on_main"
echo "stage gates: passed"
