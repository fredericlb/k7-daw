#!/bin/bash
# e2e-runner.sh — a GitHub runner's bed (Linux or Windows under Git Bash): build, one fresh dev session, then one suite.
# Run by .github/workflows/e2e-linux.yml and e2e-windows.yml from the checkout's root; mirrors e2e-macos.sh's single slot.
# The runner is the bed's own machine, thrown away after the job: no slot, no lock, no port probe, no stop.
# E2E_SUITE: web or conformance; E2E_FILTER: Playwright's file filters; E2E_MAX_FAILURES (0 for none); E2E_RETRIES.
# What a reader gets wrong: on Windows every path handed to a native program is the mixed form (C:/…), never /c/… (docs/e2e-runners.md).
set -euo pipefail

windows=0
case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) windows=1 ;; esac
if [ "$windows" = 1 ]; then
  export MSYS_NO_PATHCONV=1
  tree=$(pwd -W)
  exe=.exe
  out="$(cygpath -m "${K7_CI_WORKSPACE:?}")/e2e-out"
else
  tree=$PWD
  exe=
  out="${K7_CI_WORKSPACE:?}/e2e-out"
fi
suite=${E2E_SUITE:-web}
filter=${E2E_FILTER:-}
logs="$out/logs"
case "$suite" in web | conformance) ;; *) echo "E2E_SUITE: web or conformance, not '$suite'"; exit 2 ;; esac

front=28789 backend=28790 web=25223 engine=28765
bed="$out/../e2e-bed"
home="$bed/home"
files="$home/.k7/dev-files"
mediatek="$home/.studio/mediatek"
rm -rf "$out" "$bed"
mkdir -p "$logs" "$bed"
echo "bed: front $front, backend $backend, web $web, engine $engine, home $home"

t0=$SECONDS
lap() { echo "⏱ $1: $((SECONDS - t0)) s" | tee -a "$out/timings.txt"; t0=$SECONDS; }
# A step's log goes to the run's own output when it fails: the artifact is not the only place to read why.
step() {
  local name=$1
  shift
  "$@" > "$logs/$name.log" 2>&1 || { tail -80 "$logs/$name.log"; echo "$name failed"; exit 1; }
}

echo "=== the engine and the supervisor (${STUDIO_ENGINE_PRESET:?}) ==="
step build-engine node apps/k7-daw-engine/scripts/build-engine.mjs
lap "engine build"

echo "=== the Rust front (release) ==="
# One package per build, as dev-front.mjs builds them: together their features unify and its own builds start over.
export K7_FRONT_TARGET_DIR=${K7_FRONT_TARGET_DIR:?}
build_front() { (cd apps/k7-daw-backend && cargo build --locked --release -p k7-front --target-dir "$K7_FRONT_TARGET_DIR"); }
step build-front build_front
lap "front build"

echo "=== Playwright's Chromium and the render page ==="
step chromium npm run e2e:install --workspace @studio/e2e
step picture-page node apps/k7-daw-backend/packages/k7-picture/page.mjs
lap "chromium"

echo "=== a fresh bed ==="
mkdir -p "$files" "$mediatek" "$home/.studio/k7" "$home/AppData/Local" "$home/AppData/Roaming"
# The dev env.yaml e2e/docs/e2e.md asks for, a DEVICE's front, its token new at each run; the suites read it at the tree's root.
cat > env.yaml << EOF
# Written by apps/k7-daw-engine/scripts/ci/e2e-runner.sh for a GitHub runner's bed; rewritten at every run.
common:
  K7_SESSION_TOKEN: "$(node -p 'require("crypto").randomBytes(32).toString("hex")')"
k7-front:
  STUDIO_LOCAL_USER: "1"
  K7_INSTANCE: "local"
  STUDIO_LIBRARY_SEED: "k7-core=$tree/k7-core"
EOF
# The command-line engine, listed at the bed's front alone, downloading no instrument (apps/k7-daw-backend/packages/k7-supervisor/docs/supervisor.md).
cat > "$bed/engine.json" << EOF
{ "registryUrls": ["http://127.0.0.1:$front"], "port": $engine, "instruments": [], "machineName": "e2e-$([ "$windows" = 1 ] && echo windows || echo linux)" }
EOF

# Windows reads its home from USERPROFILE, and sccache cannot start under the bed's: there the front's cargo runs without it.
wrapper=${RUSTC_WRAPPER:-}
[ "$windows" = 0 ] || wrapper=
# cargo and rustup keep the runner's own homes: under the bed's HOME rustup finds no toolchain.
realhome=$HOME
[ "$windows" = 0 ] || realhome=$(cygpath -m "$USERPROFILE")
cargo_home=${CARGO_HOME:-$realhome/.cargo}
rustup_home=${RUSTUP_HOME:-$realhome/.rustup}
start() {
  local name=$1 dir=$2
  shift 2
  (cd "$dir" && exec env HOME="$home" USERPROFILE="$home" STUDIO_PORTS="k7=$front" RUSTC_WRAPPER="$wrapper" CARGO_HOME="$cargo_home" RUSTUP_HOME="$rustup_home" \
    STUDIO_MEDIATEK="$mediatek" STUDIO_FILES_ROOT="$files" K7_FRONT_CACHE="$bed/front-cache" "$@") > "$logs/$name.log" 2>&1 &
  echo $! > "$bed/$name.pid"
}
# The engines this run starts play to no speaker (apps/k7-daw-engine/k7-runtime/docs/audioDevice.md).
export STUDIO_ENGINE_SILENT=1
start front apps/k7-daw-backend env K7_GIT_ALLOW_FILE=1 K7_FRONT_LISTEN="127.0.0.1:$front" \
  K7_LAN_OPEN=1 K7_LAN_LISTEN="127.0.0.1:$((front + 2))" K7_LAN_PEERS=none \
  K7_GITHUB_CLIENT_ID=e2e K7_GITHUB_WEB_URL="http://127.0.0.1:$((front + 3))" K7_GITHUB_API_URL="http://127.0.0.1:$((front + 3))" node scripts/dev-front.mjs
if [ "$suite" = web ]; then
  start frontend apps/k7-daw-frontend env STUDIO_WEB_PORT=$web STUDIO_BACKEND_URL="http://127.0.0.1:$front" npx vite --host 127.0.0.1 --strictPort
  # A second front with a home of its own and no engine, serving the same page; it lists the first one's door (e2e/specs/app/lanSurfaces.spec.ts).
  mkdir -p "$bed/home-b/.k7/dev-files" "$bed/home-b/.studio/mediatek" "$bed/home-b/.studio/k7" "$bed/home-b/AppData/Local"
  start frontb apps/k7-daw-backend env HOME="$bed/home-b" USERPROFILE="$bed/home-b" STUDIO_PORTS="k7=$backend" STUDIO_MEDIATEK="$bed/home-b/.studio/mediatek" \
    STUDIO_FILES_ROOT="$bed/home-b/.k7/dev-files" K7_FRONT_CACHE="$bed/front-cache-b" K7_GIT_ALLOW_FILE=1 K7_FRONT_LISTEN="127.0.0.1:$backend" \
    K7_LAN_PEERS="127.0.0.1:$((front + 2)):$engine" K7_DEV_FRONTEND="http://127.0.0.1:$web" node scripts/dev-front.mjs
fi
# STUDIO_HOME and K7_SETTINGS_DIR: the engine's state folder is named, not derived from the OS's own folders.
start engine "$tree" env STUDIO_HOME="$home/.k7" K7_SETTINGS_DIR="$home/settings" "dist/k7-runtime/k7-supervisor$exe" --config "$bed/engine.json"

# Up when the port answers anything at all, or failed when its process is gone; curl's output goes to the shell's /dev/null.
wait_for() {
  local name=$1 url=$2 limit=$3
  for _ in $(seq "$limit"); do
    curl -sS --max-time 5 "$url" > /dev/null 2>&1 && return 0
    kill -0 "$(cat "$bed/$name.pid")" 2> /dev/null || { echo "$name ended:"; tail -60 "$logs/$name.log"; return 1; }
    sleep 1
  done
  echo "$name: no answer at $url after $limit s"; tail -60 "$logs/$name.log"; return 1
}
# The front answers before its relay runs (503 on /api/sync till then): a spec started earlier gets 409s.
relay_up() {
  local code=000
  for _ in $(seq 180); do
    code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 -H "Connection: Upgrade" -H "Upgrade: websocket" "http://127.0.0.1:$front/api/sync" 2> /dev/null || true)
    [ "$code" != 503 ] && [ "$code" != 000 ] && return 0
    sleep 1
  done
  echo "the front's relay still answers $code after 180 s:"; tail -60 "$logs/front.log"; return 1
}
wait_for front "http://127.0.0.1:$front/auth/me" 900
relay_up
[ "$suite" != web ] || { wait_for frontend "http://127.0.0.1:$web/" 180; wait_for frontb "http://127.0.0.1:$backend/auth/me" 900; }
lap "bed up"

export E2E_ENV_FILE="$tree/env.yaml"
# The engine listed for the e2e account: every project spec stops at "No engines found" otherwise.
listed=
for _ in $(seq 180); do
  listed=$(node e2e/scripts/engines.ts "http://127.0.0.1:$front" 2> /dev/null || true)
  [ -n "$listed" ] && [ "$listed" != "no engine listed" ] && break
  sleep 1
done
echo "engines: ${listed:-none}"
[ "$listed" != "no engine listed" ] && [ -n "$listed" ] || { tail -60 "$logs/engine.log"; exit 1; }
lap "engine listed"

flags=
[ "${E2E_RETRIES:-0}" = 0 ] || flags="--retries=$E2E_RETRIES"
[ "${E2E_MAX_FAILURES:-0}" = 0 ] || flags="$flags --max-failures=$E2E_MAX_FAILURES"
status=0
cd e2e
rm -rf test-results
echo "=== the $suite suite ==="
if [ "$suite" = web ]; then
  export E2E_BASE_URL="http://127.0.0.1:$web" E2E_PEER_URL="http://127.0.0.1:$backend" E2E_GITHUB_PORT=$((front + 3)) E2E_FORGEJO_PORT=$((front + 4))
  config=playwright.config.ts
else
  export E2E_BASE_URL="http://127.0.0.1:$front" CONFORMANCE_INTERNAL_SECRET=k7-dev-front CONFORMANCE_FILES_ROOT="$files" \
    CONFORMANCE_SONG_NATIVE=on CONFORMANCE_ENGINE_BIN="$tree/dist/k7-runtime/k7-runtime$exe"
  config=playwright.conformance.config.ts
fi
# A failure is counted, not fatal: the counts, the counters and the artifact still follow.
set +e
# shellcheck disable=SC2086 # the flags and the filter are lists of words
PLAYWRIGHT_HTML_OUTPUT_DIR="$out/$suite-report" PLAYWRIGHT_JSON_OUTPUT_NAME="$out/$suite.json" \
  npx playwright test -c "$config" --output "test-results/$suite" --reporter=list,html,json $flags $filter 2>&1 | tee "$logs/$suite.log"
[ "${PIPESTATUS[0]}" = 0 ] || status=1
node -e '
  const s = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).stats;
  console.log(`${process.argv[2]}: ${s.expected} passed, ${s.unexpected} failed, ${s.flaky} flaky, ${s.skipped} skipped, ${Math.round(s.duration / 1000)} s`);
' "$out/$suite.json" "$suite" | tee -a "$out/summary.txt" || status=1
cp -R "test-results/$suite" "$out/$suite-results" 2> /dev/null || true
cd "$tree"
lap "suite"
# The native counters are logged at a 30 s tick: the fronts stay up past one, then a panic or a floor not reached fails the run.
sleep 32
node scripts/agent/bedCounters.mjs ${filter:+--info} "$out" "$suite" 2>&1 | tee -a "$out/summary.txt"
[ "${PIPESTATUS[0]}" = 0 ] || status=1
node scripts/agent/bedSummary.mjs "$out" | tee -a "$out/summary.txt"
exit $status
