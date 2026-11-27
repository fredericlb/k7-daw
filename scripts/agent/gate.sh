#!/usr/bin/env bash
# gate.sh — a branch's local checks, cheapest first, stopping at the first failure (docs/agent-playbook.md).
# `--quick`: rustfmt, lint:docs, lint:bytes, biome, the generated files; with changed Rust, clippy and the Windows clippy on the affected crates.
# `--milestone`: rustfmt to the generated files, then check:changed and mac-checks (a failed test rerun alone, a load flake carried past); the Windows clippy waits for the merge.
# No flag: the merge gate, adding win-clippy, android-check and ios-check — or only a page build when nothing native changed
# (K7_GATE_NATIVE=1 forces them); `--ci-local` adds ci:local, which otherwise runs on main after merges.
# `--base <branch>` (or K7_GATE_BASE) gates a stacked branch against its parent; the long steps hold a machine-wide lock (gate-lib.sh).
# What a reader gets wrong: the long checks only start once the quick ones pass, so a slip costs seconds, not a run.
set -euo pipefail
# shellcheck source=throttle.sh
. "$(dirname "$0")/throttle.sh" "$@"
# shellcheck source=gate-lib.sh
. "$(dirname "$0")/gate-lib.sh"
args=()
while [ $# -gt 0 ]; do
  case "$1" in
    --base) [ $# -ge 2 ] || { echo "gate: --base needs a branch" >&2; exit 2; }; K7_GATE_BASE="$2"; shift 2 ;;
    *) args+=("$1"); shift ;;
  esac
done
set -- ${args[@]+"${args[@]}"}
export K7_GATE_BASE="${K7_GATE_BASE:-main}"
# JS test timeouts widen 4x when the machine starts loaded (scripts/vitestScale.ts).
gate_timeout_scale
root="$(git rev-parse --show-toplevel)"
cd "$root"
start=$SECONDS lap=$SECONDS cur="" laps=""
# Each step's seconds are printed when the next starts, and the whole list on exit, failure included.
close() { if [ -n "$cur" ]; then laps="$laps  $cur: $((SECONDS - lap))s"$'\n'; cur=""; fi; }
step() { close; cur="$1" lap=$SECONDS; echo "gate: $1"; }
# mac-checks appends the load flakes it carried past (scripts/agent/rerun-failed.sh); the summary lists them.
K7_FLAKES_FILE="$(mktemp)" && export K7_FLAKES_FILE
# The engine the vitest runs start would otherwise write the user's ~/Library/Studio/Settings.xml.
export K7_SETTINGS_DIR="${K7_SETTINGS_DIR:-$HOME/.cache/k7-settings-$(basename "$(git rev-parse --show-toplevel)")}"
mkdir -p "$K7_SETTINGS_DIR"
report() {
  close
  printf 'gate: timings\n%sgate: total %ss\n' "$laps" "$((SECONDS - start))"
  if [ -s "$K7_FLAKES_FILE" ]; then echo "gate: load flakes (passed alone, logged in ~/.cache/k7-flakes.log): $(paste -sd' ' "$K7_FLAKES_FILE")"; fi
  rm -f "$K7_FLAKES_FILE"
  gate_lock_release
}
trap report EXIT
base="$(git merge-base HEAD "$K7_GATE_BASE")"
changed() { { git diff --name-only "$base" -- "$@"; git ls-files --others --exclude-standard -- "$@"; } | sort -u | while read -r f; do if [ -f "$f" ]; then echo "$f"; fi; done; }

step "rustfmt on the changed Rust"
# Each workspace whole, as cargo reads its edition; a per-file rustfmt guessed one edition for every crate.
[ -z "$(changed '*.rs')" ] || bash scripts/lint-rustfmt.sh
step "lint:docs and lint:bytes"
npm run --silent lint:docs
npm run --silent lint:bytes
step "biome on the changed files"
js="$(changed '*.ts' '*.tsx' '*.mts' '*.mjs' '*.js' '*.json')"
if [ -n "$js" ]; then
  # shellcheck disable=SC2086 # one word per file
  npx biome check --no-errors-on-unmatched $js
fi
# A generated file is checked only when something it is built from changed.
step "generated files"
if [ -n "$(changed 'apps/k7-daw-frontend/packages/*' 'apps/k7-daw-wasm/*' 'apps/k7-daw-backend/packages/waveform/*')" ]; then
  node scripts/agent/generated-check.mjs
fi
mode="${1:-merge}"
# The crates since this branch's last milestone that passed, when it is still an ancestor; else since main.
mark="$HOME/.cache/k7-milestones/$(git symbolic-ref -q --short HEAD || echo detached)"
last="$(cat "$mark" 2> /dev/null || true)"
if [ -n "$last" ] && git merge-base --is-ancestor "$last" HEAD 2> /dev/null; then export K7_CRATES_BASE="$last"; fi
if [ "$mode" = "--quick" ]; then
  # An in-tree target folder means a --target-dir slipped after `--` (docs/agent-playbook.md "Local builds").
  if [ -d apps/k7-daw-backend/target ]; then echo "gate: apps/k7-daw-backend/target exists in $root: a cargo run built in the tree; delete it and pass --target-dir before any --" >&2; exit 1; fi
  if [ -n "$(changed '*.rs')" ]; then
    step "clippy on the affected crates"
    bash scripts/agent/mac-checks.sh --narrow --clippy
    step "win-clippy on the affected crates"
    bash scripts/agent/win-clippy.sh --narrow
  fi
  echo "gate: quick checks passed"
  exit 0
fi

step "gate lock"
gate_lock_take "$(git symbolic-ref -q --short HEAD || echo detached)"
step "check:changed"
if [ "$K7_GATE_BASE" = main ]; then npm run --silent check:changed; else npm run --silent check:changed -- --base "$K7_GATE_BASE"; fi
step "mac-checks"
if [ "$mode" = "--milestone" ]; then
  bash scripts/agent/mac-checks.sh --narrow
  mkdir -p "$(dirname "$mark")" && git rev-parse HEAD > "$mark"
  echo "gate: milestone checks passed"
  exit 0
fi
bash scripts/agent/mac-checks.sh
# Native: anything the Windows, Android or iOS builds compile beyond the page, deleted files included.
native="$(git diff --name-only "$base" -- '*.rs' '*Cargo.toml' '*Cargo.lock' '*.kt' '*.kts' '*.swift' '*.cpp' '*.h' '*.hpp' '*.mm' '*CMakeLists.txt' \
  'apps/k7-daw-android/*' 'apps/k7-daw-ios/*' 'apps/k7-daw-engine/*' 'apps/k7-daw-desktop/src-tauri/*' 'apps/k7-daw-backend/*' 'apps/k7-daw-wasm/*' \
  'k7-core/*' 'scripts/agent/*-check.sh' 'scripts/agent/win-clippy.sh' 'package-lock.json')"
if [ -z "$native" ] && [ -z "${K7_GATE_NATIVE:-}" ]; then
  # Only the page changed: the phones bundle it, so it is built once here instead of three native builds around it.
  step "page build (no native change: win-clippy, android-check and ios-check skipped)"
  out="$(mktemp -d)"
  npm exec --workspace k7-frontend -- vite build --outDir "$out" --emptyOutDir --logLevel warn
  rm -rf "$out"
else
  step "win-clippy"
  bash scripts/agent/win-clippy.sh
  step "android-check"
  bash scripts/agent/android-check.sh
  step "ios-check"
  bash scripts/agent/ios-check.sh
fi
if [ "$mode" = "--ci-local" ]; then
  step "ci:local"
  npm run ci:local
fi
echo "gate: passed"
