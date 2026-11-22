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

# How long the app's engine takes to answer whoami on its loopback socket, which the page's engine gate gives 3 s (docs/e2e-runners.md).
adb shell am start -W -n rocks.k7.daw.e2e/se.frediver.k7.MainActivity > /dev/null
adb forward tcp:8765 tcp:8765
sleep 30
(cd e2e && node -e '
  const WebSocket = require("ws");
  const once = () => new Promise((done) => {
    const ws = new WebSocket("ws://127.0.0.1:8765/");
    const t0 = Date.now();
    let sent = 0;
    const end = (what) => { try { ws.close(); } catch {} done(`${what} after ${Date.now() - t0} ms (open to reply ${sent ? Date.now() - sent : "-"} ms)`); };
    const timer = setTimeout(() => end("no reply"), 30000);
    ws.on("open", () => { sent = Date.now(); ws.send(JSON.stringify({ type: "whoami" })); });
    ws.on("message", (d) => { if (String(d).includes("whoami")) { clearTimeout(timer); end("whoami"); } });
    ws.on("error", (e) => { clearTimeout(timer); end(`error ${e.message}`); });
  });
  (async () => { for (let i = 0; i < 5; i++) console.log(`engine probe ${i + 1}: ${await once()}`); })();
') 2>&1 | tee "$logs/engine-probe.txt" || true
# The same whoami from inside the app's WebView, every event it sees: the page's own probe is the one the gate trusts.
(cd e2e && node -e '
  const { _android } = require("@playwright/test");
  (async () => {
    const [device] = await _android.devices({ omitDriverInstall: true });
    const view = await device.webView({ pkg: "rocks.k7.daw.e2e" }, { timeout: 60000 });
    // k7 is one page among the WebView'"'"'s pages, the one on the phone'"'"'s origin, as e2e/src/android.ts finds it.
    let page;
    for (let i = 0; i < 30 && !page; i++) {
      page = (await view.page()).context().pages().find((p) => p.url().startsWith("http://127.0.0.1:18799/"));
      if (!page) await new Promise((r) => setTimeout(r, 2000));
    }
    if (!page) throw new Error("k7 never showed in the WebView");
    console.log("webview page", page.url(), await page.evaluate(() => navigator.userAgent));
    for (const url of ["ws://127.0.0.1:8765/", "ws://localhost:8765/"]) {
      const said = await page.evaluate((u) => new Promise((done) => {
        const log = [];
        const t0 = performance.now();
        const at = (s) => log.push(`${Math.round(performance.now() - t0)}ms ${s}`);
        let ws;
        try { ws = new WebSocket(u); } catch (e) { return done([`threw ${e}`]); }
        ws.onopen = () => { at("open"); ws.send(JSON.stringify({ type: "whoami" })); };
        ws.onmessage = (e) => { at(`message ${String(e.data)}`); ws.close(); };
        ws.onerror = () => at("error");
        ws.onclose = (e) => { at(`close ${e.code} ${e.reason}`); done(log); };
        setTimeout(() => { at("timeout"); done(log); }, 10000);
      }), url);
      console.log(`webview probe ${url}: ${said.join(" | ")}`);
    }
    await device.close();
  })().catch((e) => console.log(`webview probe failed: ${e.message}`));
') 2>&1 | tee -a "$logs/engine-probe.txt" || true
# The id the engine registered under, which the page's probe matches the whoami against.
adb logcat -d | grep -oE "\[registry\] engine [0-9a-f-]+" | sort -u | tee -a "$logs/engine-probe.txt" || true

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
