#!/usr/bin/env bash
# ios-check.sh — what a branch passes for iOS on the Mac before it merges: k7-embed built as a static library for both iOS targets, then the app built for the simulator against the staged frameworks.
# The C++ engine and supervisor are not built: the staged frameworks of the last `stage.mjs --sim` are linked as they are (apps/k7-daw-engine/k7-runtime/docs/ios.md).
# What a reader gets wrong: Xcode links the library built HERE (a folder of its own, never the staged release one), so a missing symbol shows, and the suite's app in ~/.cache/k7-ios-dd is left alone.
set -euo pipefail
# shellcheck source=throttle.sh
. "$(dirname "$0")/throttle.sh" "$@"
root="$(git rev-parse --show-toplevel)"
cd "$root"
export PATH="$HOME/.cargo/bin:$PATH"
# What the app's build reads: a branch touching none of it since main has nothing to check here.
reads=(apps/k7-daw-backend apps/k7-daw-frontend apps/k7-daw-wasm k7-core apps/k7-daw-ios package-lock.json scripts/agent/ios-check.sh)
if [ "${1:-}" != "--all" ] && git diff --quiet "$(git merge-base HEAD main)" -- "${reads[@]}" && [ -z "$(git ls-files --others --exclude-standard -- "${reads[@]}")" ]; then
  echo "ios-check: nothing the app's build reads changed since main, skipped (--all runs it)"
  exit 0
fi
staged="$root/apps/k7-daw-ios/build/staged"
# A slot without its own staged frameworks links another slot's (K7_IOS_STAGED): the project reads them at this path.
if [ ! -e "$staged" ] && [ -n "${K7_IOS_STAGED:-}" ]; then mkdir -p "$(dirname "$staged")" && ln -s "$K7_IOS_STAGED" "$staged"; fi
[ -d "$staged/frameworks/k7runtime.xcframework" ] && [ -d "$staged/frameworks/k7supervisor.xcframework" ] || { echo "ios-check: no staged frameworks: run node apps/k7-daw-ios/scripts/stage.mjs --sim first" >&2; exit 2; }
eval "$(node apps/k7-daw-engine/scripts/sccache.mjs agent --export)"
# One target folder for every slot: cargo locks it, so two checks wait rather than build twice cold.
target_dir="$HOME/.cache/k7-ios-check"
lib_dir="$target_dir/lib"
start=$SECONDS
mkdir -p "$lib_dir"
# QuickJS is gone from the phone: no crate of the front may bring it back, whatever it is linked for.
if (cd apps/k7-daw-backend && cargo tree --target aarch64-apple-ios -p k7-embed -i rquickjs-sys >/dev/null 2>&1); then
  echo "ios-check: k7-embed depends on rquickjs-sys again (cargo tree -i rquickjs-sys finds it)" >&2
  exit 1
fi
for target in aarch64-apple-ios aarch64-apple-ios-sim; do
  echo "ios-check: k7-embed for $target"
  # The same crate type and deployment target as stage.mjs, in the dev profile: no LTO, so the check costs minutes.
  (cd apps/k7-daw-backend && IPHONEOS_DEPLOYMENT_TARGET=17.0 cargo rustc -p k7-embed --target "$target" --crate-type staticlib --target-dir "$target_dir")
done
cp "$target_dir/aarch64-apple-ios-sim/debug/libk7_embed.a" "$lib_dir/libk7_embed.a"
if nm "$lib_dir/libk7_embed.a" 2>/dev/null | grep -q JS_NewRuntime; then
  echo "ios-check: libk7_embed.a holds QuickJS (JS_NewRuntime)" >&2
  exit 1
fi
echo "ios-check: xcodebuild for the simulator"
# Signing is off (a simulator build needs none); the library folder is overridden so the fresh library is the one linked.
xcodebuild -project apps/k7-daw-ios/k7.xcodeproj -scheme k7 -configuration Debug \
  -destination "generic/platform=iOS Simulator" -derivedDataPath "$target_dir/dd" \
  CODE_SIGNING_ALLOWED=NO LIBRARY_SEARCH_PATHS="$lib_dir" -quiet build
echo "ios-check: passed in $((SECONDS - start))s"
