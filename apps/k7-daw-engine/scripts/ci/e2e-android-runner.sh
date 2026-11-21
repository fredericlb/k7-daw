#!/bin/bash
# e2e-android-runner.sh — the web suite's specs on an Android emulator: the app's e2e build, installed and driven by playwright.android.config.ts.
# Run by .github/workflows/e2e-android.yml inside the emulator runner's script, once the APK is built: it installs that APK, the global setup builds nothing.
# The emulator is x86_64 and the app arm64-v8a only: Android's own ARM translation runs it (docs/e2e-runners.md).
# E2E_FILTER: Playwright's file filters; E2E_MAX_FAILURES (0 for none); E2E_RETRIES.
set -euo pipefail

tree=$PWD
filter=${E2E_FILTER:-}
out="${K7_CI_WORKSPACE:?}/e2e-out"
logs="$out/logs"
rm -rf "$out"
mkdir -p "$logs"

adb wait-for-device
adb shell getprop ro.product.cpu.abilist | tee "$logs/abilist.txt"
apk=$(ls apps/k7-daw-android/app/build/outputs/apk/e2e/*.apk | head -1)
echo "installing $apk"
adb install -r -g "$apk"
export E2E_ANDROID_BUILD=0
# The phone's whole log, the front's and the engine's lines among it (apps/k7-daw-engine/k7-runtime/docs/android.md).
adb logcat -c || true
adb logcat -v time > "$logs/logcat.txt" 2>&1 &
trap 'kill $! 2> /dev/null || true' EXIT

flags=
[ "${E2E_RETRIES:-0}" = 0 ] || flags="--retries=$E2E_RETRIES"
[ "${E2E_MAX_FAILURES:-0}" = 0 ] || flags="$flags --max-failures=$E2E_MAX_FAILURES"
status=0
cd e2e
rm -rf test-results
set +e
# shellcheck disable=SC2086 # the flags and the filter are lists of words
PLAYWRIGHT_HTML_OUTPUT_DIR="$out/android-report" PLAYWRIGHT_JSON_OUTPUT_NAME="$out/android.json" \
  npx playwright test -c playwright.android.config.ts --reporter=list,html,json $flags $filter 2>&1 | tee "$logs/android.log"
[ "${PIPESTATUS[0]}" = 0 ] || status=1
node -e '
  const s = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).stats;
  console.log(`android: ${s.expected} passed, ${s.unexpected} failed, ${s.flaky} flaky, ${s.skipped} skipped, ${Math.round(s.duration / 1000)} s`);
' "$out/android.json" | tee -a "$out/summary.txt"
[ "${PIPESTATUS[0]}" = 0 ] || status=1
cp -R test-results/android "$out/android-results" 2> /dev/null
cd "$tree"
exit $status
