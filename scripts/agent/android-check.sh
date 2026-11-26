#!/usr/bin/env bash
# android-check.sh — what a branch passes for Android on the Mac before it merges: the Rust front and supervisor built for the phone, the bundled backend staged, the app's Kotlin compiled.
# The C++ engine is not built (the port does not touch it): a change there needs the full APK (apps/k7-daw-engine/k7-runtime/docs/android.md).
# What a reader gets wrong: CI does not build Android for a branch any more (docs/agent-playbook.md); this is the check.
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export JAVA_HOME="${JAVA_HOME:-$(/usr/libexec/java_home -v 21)}"
[ -d "$ANDROID_HOME/ndk" ] || { echo "no Android SDK with an NDK at $ANDROID_HOME" >&2; exit 2; }
cd "$root"
# What the phone's build reads: a branch touching none of it since main has nothing to check here.
reads=(apps/k7-daw-backend apps/k7-daw-frontend apps/k7-daw-wasm k7-core apps/k7-daw-android e2e package-lock.json scripts/agent/android-check.sh)
if [ "${1:-}" != "--all" ] && git diff --quiet "$(git merge-base HEAD main)" -- "${reads[@]}" && [ -z "$(git ls-files --others --exclude-standard -- "${reads[@]}")" ]; then
  echo "android-check: nothing the phone's build reads changed since main, skipped (--all runs it)"
  exit 0
fi
# The suite's node verb runner reaches the phone's front only when the android config names it (e2e/docs/e2e.md).
( cd "$root/e2e" && env -u E2E_BASE_URL "$root/node_modules/.bin/tsx" --eval '(async () => {
  const { PHONE_ORIGIN } = await import("./src/android.ts");
  await import("./playwright.android.config.ts");
  if (process.env.E2E_BASE_URL !== PHONE_ORIGIN) throw new Error(`the android suite runs its verbs against ${process.env.E2E_BASE_URL}, not ${PHONE_ORIGIN}`);
})();' )
cd "$root/apps/k7-daw-android"
# The configuration cache skips Gradle's configuring phase when the build scripts did not change.
./gradlew --no-daemon --configuration-cache :app:buildFront :app:stageSupervisor :app:stageBackend :app:compileE2eKotlin \
  -x buildEngine -x stageEngine
echo "android-check: passed"
