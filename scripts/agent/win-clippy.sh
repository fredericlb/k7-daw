#!/usr/bin/env bash
# win-clippy.sh — clippy for x86_64-pc-windows-gnu on a Mac or Linux, before a ci/ push: what only Windows compiles (a cfg-gated import, a Windows-only API) fails here in a minute.
# `--narrow` checks the backend crates touched since K7_CRATES_BASE and their dependents, and the browser crates only when the branch touched them.
# It checks and never links or runs: a test that fails only on Windows still needs OneDev's "rust checks windows" (docs/rust-builds.md).
set -euo pipefail
# The tree checked is the one it is run from, so one copy of the script serves every worktree.
root="$(git rev-parse --show-toplevel)"
bin="$(ls -d "$HOME"/.cache/llvm-mingw/llvm-mingw-*/bin 2>/dev/null | tail -1)"
[ -n "$bin" ] || { echo "no llvm-mingw under ~/.cache/llvm-mingw: see docs/rust-builds.md" >&2; exit 2; }
rustup target add x86_64-pc-windows-gnu > /dev/null
export CC_x86_64_pc_windows_gnu="$bin/x86_64-w64-mingw32-clang" CXX_x86_64_pc_windows_gnu="$bin/x86_64-w64-mingw32-clang++" AR_x86_64_pc_windows_gnu="$bin/llvm-ar"
eval "$(node "$root/engine/scripts/sccache.mjs" agent --export 2>/dev/null || node "$root/engine/scripts/sccache.mjs" dev --export)"
target="$HOME/.cache/k7-wincheck-$(basename "$root")"
win=(--locked --all-targets --target x86_64-pc-windows-gnu --target-dir "$target")
crates="--workspace" areas="backend browser"
if [ "${1:-}" = "--narrow" ]; then
  crates="$(cd "$root" && node scripts/agent/affected-crates.mjs)"
  areas="$(cd "$root" && K7_AREAS_BASE=main node engine/scripts/ci/areas.mjs)"
fi
# shellcheck disable=SC2086 # one word per -p flag
[ -z "$crates" ] || (cd "$root/apps/k7/backend-rs" && cargo clippy "${win[@]}" $crates -- -D warnings)
[[ " $areas " == *" browser "* ]] || { echo "win-clippy: clean (no browser crate touched)"; exit 0; }
for crate in packages/song-schema/rust packages/k7-wasm/rust packages/song/rust; do
  (cd "$root/$crate" && cargo clippy "${win[@]}" --features ts -- -D warnings)
done
(cd "$root/packages/k7x/rust" && cargo clippy "${win[@]}" -- -D warnings)
(cd "$root/packages/song-edit/rust" && cargo clippy "${win[@]}" -- -D warnings)
echo "win-clippy: clean"
