#!/bin/bash
# e2e-macos.sh — the Mac bed: a dev session of its own, then the web and conformance suites.
# Run by scripts/agent/bed.sh from a worktree; the script tests the tree it runs in.
# The session never shares a port, a home or an engine with what the Mac runs otherwise: its own ports and a HOME under
# ~/k7-trees/e2e-macos-bed, wiped at each run, paired with nothing (e2e/docs/e2e.md, "A second place to run the suites").
# E2E_SUITE: web, conformance or both; E2E_FILTER: Playwright's file filters, space-separated; E2E_MAX_FAILURES (10, 0 for none).
# What a reader gets wrong: `both` brings up TWO beds, slot 1 for the web suite and slot 2 for the conformance one, run side by side.
set -euo pipefail

tree=$PWD
suite=${E2E_SUITE:-web}
filter=${E2E_FILTER:-}
out="${K7_CI_WORKSPACE:?}/e2e-out"
logs="$out/logs"
case "$suite" in web | conformance | both) ;; *) echo "E2E_SUITE: web, conformance or both, not '$suite'"; exit 2 ;; esac
listening() { for p in "$@"; do lsof -nP -iTCP:"$p" -sTCP:LISTEN > /dev/null 2>&1 && return 0; done; return 1; }
# A lock is a folder holding its owner's pid: one whose owner is gone (a run killed outright) is taken over.
lock() {
  if ! mkdir "$1" 2> /dev/null; then
    kill -0 "$(cat "$1/pid" 2> /dev/null || echo 0)" 2> /dev/null && return 1
    rm -rf "$1"
    mkdir "$1" 2> /dev/null || return 1
  fi
  echo $$ > "$1/pid"
}
unlock() { [ "$(cat "$1/pid" 2> /dev/null)" = $$ ] && rm -rf "$1"; return 0; }
ports_of() { local o=$(( ($1 - 1) * 100 )); echo $((28789 + o)) $((28790 + o)) $((25223 + o)) $((28765 + o)); }
# A slot is free when its ports are silent; its lock, taken after, says a living run holds it.
free_slot() {
  # shellcheck disable=SC2046 # the four ports are words
  ! listening $(ports_of "$1")
}
slots=()
deps_lock="$STUDIO_FETCHCONTENT_DIR/.e2e-engine-build.lock"
release_slots() { local n; for n in ${slots[@]+"${slots[@]}"}; do unlock "$HOME/k7-trees/e2e-macos-slot-$n.lock"; done; }
trap 'unlock "$deps_lock"; release_slots' EXIT
# `both` takes slots 1 and 2 together; one suite takes the first free of E2E_BED (1 by default).
for _ in $(seq 480); do
  if [ "$suite" = both ]; then
    if free_slot 1 && free_slot 2 && lock "$HOME/k7-trees/e2e-macos-slot-1.lock"; then
      if lock "$HOME/k7-trees/e2e-macos-slot-2.lock"; then slots=(1 2); break; fi
      unlock "$HOME/k7-trees/e2e-macos-slot-1.lock"
    fi
  else
    for n in ${E2E_BED:-1}; do
      free_slot "$n" && lock "$HOME/k7-trees/e2e-macos-slot-$n.lock" && { slots=("$n"); break; }
    done
    [ "${#slots[@]}" = 0 ] || break
  fi
  sleep 30
done
[ "${#slots[@]}" -gt 0 ] || { echo "no bed slot came free in four hours"; exit 1; }

# One bed's names: its ports, its folder and its logs; `bed_of <slot>` sets them for what follows.
bed_of() {
  read -r front backend web engine <<< "$(ports_of "$1")"
  bed="$HOME/k7-trees/e2e-macos-bed$([ "$1" = 1 ] || echo "-$1")"
  home="$bed/home"
  files="$home/.k7/dev-files"
  mediatek="$home/.studio/mediatek"
  # The first bed logs where the summary and the counters read; a second bed under conformance-bed/.
  blogs=$logs
  [ "$1" = "${slots[0]}" ] || blogs="$out/conformance-bed/logs"
}

rm -rf "$out"
mkdir -p "$logs"

# Every process of the beds: those they started, by group, then whatever still runs from this tree (the engine detaches).
stop_bed() {
  set +e
  local n f
  for n in "${slots[@]}"; do
    bed_of "$n"
    [ -e "$bed/yield.pid" ] && kill "$(cat "$bed/yield.pid")" 2> /dev/null && rm -f "$bed/yield.pid"
    for f in "$bed"/*.pgid; do
      [ -e "$f" ] || continue
      kill -TERM -- "-$(cat "$f")" 2> /dev/null
      rm -f "$f"
    done
  done
  pkill -TERM -f "$tree/" 2> /dev/null
  sleep 2
  pkill -KILL -f "$tree/" 2> /dev/null
  # Stopped means the ports are free: the next run (or the next agent's) must not meet a half-dead listener.
  for n in "${slots[@]}"; do
    # shellcheck disable=SC2046 # the four ports are words
    for _ in $(seq 10); do listening $(ports_of "$n") || break; sleep 1; done
  done
  return 0
}
trap 'stop_bed; unlock "$deps_lock"; release_slots' EXIT
trap 'exit 130' INT TERM
# A run killed outright left its bed running: the tree's lock says no other run uses it.
stop_bed
set -e

for n in "${slots[@]}"; do
  bed_of "$n"
  mkdir -p "$blogs" "$bed"
  echo "bed slot $n: front $front, backend $backend, web $web, engine $engine"
  # Nothing of the Mac's answers on the bed's ports: a listener there would shadow the bed's loopback bind.
  for p in $front $backend $web $engine $((front + 2)); do
    if lsof -nP -iTCP:"$p" -sTCP:LISTEN > /dev/null 2>&1; then
      echo "port $p is held on this Mac:"; lsof -nP -iTCP:"$p" -sTCP:LISTEN; exit 1
    fi
  done
done

# A sleeping Mac freezes the app and the engine mid-spec (e2e/docs/e2e.md).
caffeinate -dims -w $$ &

t0=$SECONDS
lap() { echo "⏱ $1: $((SECONDS - t0)) s" | tee -a "$out/timings.txt"; t0=$SECONDS; }

echo "=== the engine and the supervisor (macos-clang) ==="
# Two beds take the shared dependencies in turn.
until lock "$deps_lock"; do sleep 10; done
export CMAKE_BUILD_PARALLEL_LEVEL=8 CARGO_BUILD_JOBS=8
built=0
STUDIO_ENGINE_PRESET=macos-clang STUDIO_SUPERVISOR_PRESET=macos-clang node apps/k7-daw-engine/scripts/build-engine.mjs > "$logs/build-engine.log" 2>&1 || built=1
unlock "$deps_lock"
[ "$built" = 0 ] || { tail -60 "$logs/build-engine.log"; exit 1; }
lap "engine build"

echo "=== the Rust front (release) ==="
# One package per build, as dev-front.mjs builds them: together their features unify and its own builds start over.
# The build folder is under ~/.cache, one per tree: a target inside the worktree makes gate.sh refuse it.
export K7_FRONT_TARGET_DIR="$HOME/.cache/k7-macbed-$(basename "$tree")"
(cd apps/k7-daw-backend && cargo build --locked --release -p k7-front --target-dir "$K7_FRONT_TARGET_DIR") > "$logs/build-front.log" 2>&1 ||
  { tail -60 "$logs/build-front.log"; exit 1; }
lap "front build"

# The conformance suite needs both too: picture render drives this Chromium, with the page built here.
echo "=== Playwright's Chromium, kept for this tree, and the render page ==="
export PLAYWRIGHT_BROWSERS_PATH="$HOME/k7-trees/e2e-macos-browsers"
npm run e2e:install --workspace @studio/e2e > "$logs/chromium.log" 2>&1 || { tail -30 "$logs/chromium.log"; exit 1; }
node apps/k7-daw-backend/packages/k7-picture/page.mjs >> "$logs/chromium.log" 2>&1 || { tail -30 "$logs/chromium.log"; exit 1; }
lap "chromium"

echo "=== a fresh bed ==="
# The dev env.yaml e2e/docs/e2e.md asks for, a DEVICE's front (STUDIO_LOCAL_USER, as the desktop app sets it), its token new at each run; the suites read it at the tree's root.
# It holds what every bed shares: a key it names beats the environment, so each bed's folders reach its processes as variables instead.
cat > env.yaml << EOF
# Written by apps/k7-daw-engine/scripts/ci/e2e-macos.sh for the macOS e2e bed; rewritten at every run.
common:
  K7_SESSION_TOKEN: "$(openssl rand -hex 32)"
k7-front:
  STUDIO_LOCAL_USER: "1"
  K7_INSTANCE: "local"
  STUDIO_LIBRARY_SEED: "k7-core=$tree/k7-core"
EOF

# Each one in a group of its own (set -m), so the stop reaches its children; cargo and rustup keep the agent's own homes.
# STUDIO_PORTS names k7's loopback port, which the supervisor otherwise reads as 8789 in any loopback registry URL.
start() {
  local name=$1 dir=$2
  shift 2
  (cd "$dir" && exec env HOME="$home" STUDIO_PORTS="k7=$front" CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup" \
    STUDIO_MEDIATEK="$mediatek" STUDIO_FILES_ROOT="$files" K7_FRONT_CACHE="$bed/front-cache" "$@") > "$blogs/$name.log" 2>&1 &
  echo $! > "$bed/$name.pgid"
}
# The engines this run starts play to no speaker; set K7_BED_SOUND=1 to hear them (apps/k7-daw-engine/k7-runtime/docs/audioDevice.md).
[ "${K7_BED_SOUND:-0}" = 1 ] || export STUDIO_ENGINE_SILENT=1

# Up when the port answers anything at all, or failed when its process is gone.
wait_for() {
  local name=$1 url=$2 limit=$3
  for _ in $(seq "$limit"); do
    curl -sS -o /dev/null --max-time 5 "$url" 2> /dev/null && return 0
    kill -0 "$(cat "$bed/$name.pgid")" 2> /dev/null || { echo "$name ended:"; tail -40 "$blogs/$name.log"; return 1; }
    sleep 1
  done
  echo "$name: no answer at $url after $limit s"; tail -40 "$blogs/$name.log"; return 1
}
# The front answers before its relay runs (503 on /api/sync till then): a spec started earlier gets 409s, so the bed waits and fails fast.
relay_up() {
  for _ in $(seq 180); do
    code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 -H "Connection: Upgrade" -H "Upgrade: websocket" "http://127.0.0.1:$front/api/sync" 2> /dev/null || true)
    [ "$code" != 503 ] && [ "$code" != 000 ] && return 0
    sleep 1
  done
  echo "the front's relay still answers $code after 180 s:"; tail -40 "$blogs/front.log"; return 1
}

# The suite a slot serves: the web one on the first slot unless the run is conformance alone.
suite_of() { if [ "$suite" = both ]; then [ "$1" = "${slots[0]}" ] && echo web || echo conformance; else echo "$suite"; fi; }

# One bed: wiped, started, then up once its front relays and, for the web suite, its Vite answers.
bring_up() {
  local n=$1 serves
  serves=$(suite_of "$n")
  bed_of "$n"
  rm -rf "$home"
  mkdir -p "$files" "$mediatek" "$home/.studio/k7"
  # The command-line engine, listed at the bed's front alone, downloading no instrument (apps/k7-daw-engine/k7-supervisor/docs/supervisor.md).
  cat > "$bed/engine.json" << EOF
{ "registryUrls": ["http://127.0.0.1:$front"], "port": $engine, "instruments": [], "machineName": "e2e-macos$([ "$n" = 1 ] || echo "-$n")" }
EOF
  set -m
  # The LAN door on the loopback beside the front, the switch on and no discovery: a second front names it below (docs/lan.md).
  start front apps/k7-daw-backend env K7_GIT_ALLOW_FILE=1 K7_FRONT_LISTEN="127.0.0.1:$front" \
    K7_LAN_OPEN=1 K7_LAN_LISTEN="127.0.0.1:$((front + 2))" K7_LAN_PEERS=none node scripts/dev-front.mjs
  if [ "$serves" = web ]; then
    start frontend apps/k7-daw-frontend env STUDIO_WEB_PORT=$web STUDIO_BACKEND_URL="http://127.0.0.1:$front" npx vite --host 127.0.0.1 --strictPort
    # A second front with a home of its own and no engine, serving the same page; it lists the first one's door (e2e/specs/app/lanSurfaces.spec.ts).
    mkdir -p "$bed/home-b/.k7/dev-files" "$bed/home-b/.studio/mediatek" "$bed/home-b/.studio/k7"
    start frontb apps/k7-daw-backend env HOME="$bed/home-b" STUDIO_PORTS="k7=$backend" STUDIO_MEDIATEK="$bed/home-b/.studio/mediatek" \
      STUDIO_FILES_ROOT="$bed/home-b/.k7/dev-files" K7_FRONT_CACHE="$bed/front-cache-b" K7_GIT_ALLOW_FILE=1 K7_FRONT_LISTEN="127.0.0.1:$backend" \
      K7_LAN_PEERS="127.0.0.1:$((front + 2)):$engine" K7_DEV_FRONTEND="http://127.0.0.1:$web" node scripts/dev-front.mjs
  fi
  # STUDIO_HOME and K7_SETTINGS_DIR: the engine finds its home through macOS, not HOME, and would share the Mac's ~/.k7 and Library/Studio.
  start engine "$tree" env STUDIO_HOME="$home/.k7" K7_SETTINGS_DIR="$home/settings" dist/k7-runtime/k7-supervisor --config "$bed/engine.json"
  set +m
}
for n in "${slots[@]}"; do bring_up "$n"; done
for n in "${slots[@]}"; do
  bed_of "$n"
  wait_for front "http://127.0.0.1:$front/auth/me" 300
  relay_up
  [ "$(suite_of "$n")" != web ] || { wait_for frontend "http://127.0.0.1:$web/" 120; wait_for frontb "http://127.0.0.1:$backend/auth/me" 300; }
done
lap "bed up"

export E2E_ENV_FILE="$tree/env.yaml"
# The engine listed for the e2e account: every project spec stops at "No engines found" otherwise.
for n in "${slots[@]}"; do
  bed_of "$n"
  for _ in $(seq 120); do
    listed=$(node e2e/scripts/engines.ts "http://127.0.0.1:$front" 2> /dev/null || true)
    [ -n "$listed" ] && [ "$listed" != "no engine listed" ] && break
    sleep 1
  done
  echo "engines (slot $n): ${listed:-none}"
  [ "$listed" != "no engine listed" ] && [ -n "$listed" ] || { tail -40 "$blogs/engine.log"; exit 1; }
done
lap "engine listed"
# While the suites run, this user's compilers yield the cores to the bed: renders are timed (docs/rust-builds.md).
bed_of "${slots[0]}"
(
  while kill -0 $$ 2> /dev/null; do
    pids=$(pgrep -u "$(id -u)" -x 'rustc|cargo|sccache|ld|clang|cc' || true)
    [ -z "$pids" ] || renice 15 -p $pids > /dev/null 2>&1 || true
    sleep 15
  done
) &
echo $! > "$bed/yield.pid"

# Playwright's numbers from its JSON report, one line per suite.
count() {
  node -e '
    const s = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).stats;
    console.log(`${process.argv[2]}: ${s.expected} passed, ${s.unexpected} failed, ${s.flaky} flaky, ${s.skipped} skipped, ${Math.round(s.duration / 1000)} s`);
  ' "$1" "$2" | tee -a "$out/summary.txt"
}

status=0
retries=$(bash "$tree/apps/k7-daw-engine/scripts/ci/bed-retries.sh")
# A broken branch stops at its tenth failure rather than waiting out every test's timeout (e2e/docs/e2e.md).
max_failures=${E2E_MAX_FAILURES:-10}
[ "$max_failures" = 0 ] || retries="$retries --max-failures=$max_failures"
# One suite against one bed; its own seconds go to timings.txt, since the two suites of `both` run at once.
# shellcheck disable=SC2086 # the flags and the filter are lists of words
run_suite() {
  local name=$1 config=$2 began=$SECONDS rc=0
  shift 2
  PLAYWRIGHT_HTML_OUTPUT_DIR="$out/$name-report" PLAYWRIGHT_JSON_OUTPUT_NAME="$out/$name.json" \
    npx playwright test -c "$config" --output "test-results/$name" --reporter=list,html,json $retries $filter > "$logs/$name.log" 2>&1 || rc=1
  echo "⏱ $name suite: $((SECONDS - began)) s" >> "$out/timings.txt"
  return $rc
}
web_suite() {
  bed_of "${slots[0]}"
  E2E_BASE_URL="http://127.0.0.1:$web" E2E_PEER_URL="http://127.0.0.1:$backend" run_suite web playwright.config.ts
}
conformance_suite() {
  bed_of "${slots[${#slots[@]} - 1]}"
  E2E_BASE_URL="http://127.0.0.1:$front" CONFORMANCE_INTERNAL_SECRET=k7-dev-front \
    CONFORMANCE_FILES_ROOT="$files" \
    CONFORMANCE_SONG_NATIVE=on CONFORMANCE_ENGINE_BIN="$tree/dist/k7-runtime/k7-runtime" \
    run_suite conformance playwright.conformance.config.ts
}

cd e2e
rm -rf test-results
pids=()
names=()
if [ "$suite" != conformance ]; then echo "=== the web suite ==="; web_suite & pids+=($!); names+=(web); fi
if [ "$suite" != web ]; then echo "=== the conformance suite ==="; conformance_suite & pids+=($!); names+=(conformance); fi
for i in "${!pids[@]}"; do wait "${pids[$i]}" || status=1; done
for name in "${names[@]}"; do
  echo "=== the $name suite: its end ==="
  tail -40 "$logs/$name.log"
  count "$out/$name.json" "$name" || status=1
done
[ "$suite" = conformance ] || cp -R test-results/web "$out/web-results" 2> /dev/null || true
cd "$tree"
lap "suites"
# The native counters are logged at a 30 s tick: the fronts stay up past one, then a panic or a floor not reached fails the run (e2e/docs/e2e.md).
sleep 32
if [ "$suite" = both ]; then
  node scripts/agent/bedCounters.mjs ${filter:+--info} "$out" web 2>&1 | tee -a "$out/summary.txt"
  [ "${PIPESTATUS[0]}" = 0 ] || status=1
  node scripts/agent/bedCounters.mjs ${filter:+--info} "$out/conformance-bed" conformance 2>&1 | tee -a "$out/summary.txt"
  [ "${PIPESTATUS[0]}" = 0 ] || status=1
else
  node scripts/agent/bedCounters.mjs ${filter:+--info} "$out" "$suite" 2>&1 | tee -a "$out/summary.txt"
  [ "${PIPESTATUS[0]}" = 0 ] || status=1
fi
node "$tree/scripts/agent/bedSummary.mjs" "$out" | tee -a "$out/summary.txt" || true
exit $status
