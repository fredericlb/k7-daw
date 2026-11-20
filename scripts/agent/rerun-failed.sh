# rerun-failed.sh — sourced by mac-checks.sh: a nextest run that reruns its failed tests alone and tells a load flake from a real failure.
# A failed test that passes alone, in a file the branch did not change, is a load flake: logged, printed, and the gate goes on.
# What a reader gets wrong: a test in a changed file is never forgiven by a pass alone, since it is the branch's to explain.

# The files the branch changed since main, one per line (K7_CHANGED_FILES overrides, for the harness).
rerun_changed_files() {
  if [ -n "${K7_CHANGED_FILES+x}" ]; then printf '%s\n' "$K7_CHANGED_FILES"; return; fi
  local base
  base="$(git merge-base HEAD "${K7_GATE_BASE:-main}" 2> /dev/null || echo HEAD)"
  { git diff --name-only "$base"; git ls-files --others --exclude-standard; } 2> /dev/null | sort -u
}

# Whether a failed test (`binary test`) is defined in a changed .rs file: the file at its module path, or its `fn`.
rerun_in_changed_file() {
  local bin="${1%% *}" name="${1#* }" files="$2" f s base mp="" target="" short
  local crate="${bin%%::*}"
  local -a sufs=()
  short="${name##*::}"
  if [[ "$name" == *::* ]]; then mp="${name%::*}"; mp="${mp//::/\/}"; fi
  if [[ "$bin" == *::* && "$bin" != *::bin/* ]]; then target="${bin#*::}"; fi
  if [ -n "$target" ]; then
    base="packages/$crate/tests"
    sufs=("$base/$target.rs" "$base/$target/main.rs")
    if [ -n "$mp" ]; then sufs+=("$base/$target/$mp.rs" "$base/$target/$mp/mod.rs"); fi
  else
    base="packages/$crate/src"
    if [ -n "$mp" ]; then sufs=("$base/$mp.rs" "$base/$mp/mod.rs"); else sufs=("$base/lib.rs" "$base/main.rs"); fi
  fi
  while IFS= read -r f; do
    case "$f" in *.rs) ;; *) continue ;; esac
    for s in "${sufs[@]}"; do
      if [[ "$f" == "$s" || "$f" == */"$s" ]]; then return 0; fi
    done
    if [ -f "$f" ] && grep -q "fn $short\b" "$f" 2> /dev/null; then return 0; fi
  done <<< "$files"
  return 1
}

# The `binary_id test` pairs of the FAIL lines in a nextest log, one per line, each once.
rerun_failed_in() {
  grep -E '^[[:space:]]+(FAIL|SIGABRT|SIGSEGV|SIGBUS|TIMEOUT|LEAK-FAIL|ABORT)[[:space:]]+\[' "$1" |
    sed -E 's/^[^]]*\][[:space:]]+(\([[:space:]]*[0-9]+\/[0-9]+\)[[:space:]]+)?//' | awk 'NF >= 2 { print $1 " " $2 }' | sort -u
}

# The nextest filter that selects exactly the given `binary_id test` pairs.
rerun_filter() {
  local line expr="" bin test
  while IFS= read -r line; do
    bin="${line%% *}" test="${line#* }"
    expr="${expr:+$expr | }(binary_id(=$bin) & test(=$test))"
  done <<< "$1"
  printf '%s' "$expr"
}

# The 1-minute load average, as an integer; empty when the machine does not say.
rerun_load() {
  if [ -n "${K7_LOAD+x}" ]; then printf '%s' "$K7_LOAD"; return; fi
  { sysctl -n vm.loadavg 2> /dev/null || cat /proc/loadavg 2> /dev/null; } | tr -d '{}' | awk '{ printf "%d", $1 }'
}

# Polls the load every K7_QUIET_POLL s (10) up to K7_QUIET_MAX s (180) until it is below the CPU count, so the rerun is not itself a load victim.
rerun_wait_quiet() {
  local cpus waited=0 load
  cpus="$(getconf _NPROCESSORS_ONLN 2> /dev/null || sysctl -n hw.ncpu)"
  while :; do
    load="$(rerun_load)"
    if [ -z "$load" ] || [ "$load" -lt "$cpus" ]; then return 0; fi
    if [ "$waited" -ge "${K7_QUIET_MAX:-180}" ]; then echo "gate: gave up waiting for a quiet machine (load $load, $cpus cpus), rerunning anyway"; return 0; fi
    echo "gate: waiting for a quiet machine before the rerun (load $load, $cpus cpus)"
    sleep "${K7_QUIET_POLL:-10}"
    waited=$((waited + ${K7_QUIET_POLL:-10}))
  done
}

# rerun_nextest <nextest run args>: the run; on failure the failed tests alone, one thread, up to twice each.
rerun_nextest() {
  local log rc=0 failed names="" changed bad="" flakes="" attempt line name still
  log="$(mktemp)"
  # K7_NEXTEST_FILTER narrows the first run only: a rerun names the failed tests itself, and a second -E would be OR-ed in.
  cargo nextest run --color never ${K7_NEXTEST_FILTER:+-E "$K7_NEXTEST_FILTER"} "$@" 2>&1 | tee "$log" || rc=${PIPESTATUS[0]}
  if [ "$rc" -eq 0 ]; then rm -f "$log"; return 0; fi
  failed="$(rerun_failed_in "$log")"
  rm -f "$log"
  if [ -z "$failed" ]; then echo "gate: the test run failed without a failed test (a build error?)" >&2; return "$rc"; fi
  changed="$(rerun_changed_files)"
  rerun_wait_quiet
  for attempt in 1 2; do
    echo "gate: rerunning alone (attempt $attempt): $(printf '%s' "$failed" | awk '{print $2}' | paste -sd' ' -)"
    log="$(mktemp)"
    rc=0
    cargo nextest run --color never "$@" -E "$(rerun_filter "$failed")" --test-threads 1 2>&1 | tee "$log" || rc=${PIPESTATUS[0]}
    if [ "$rc" -eq 0 ]; then
      flakes="$flakes"$'\n'"$failed"
      failed=""
    else
      still="$(rerun_failed_in "$log")"
      [ -n "$still" ] || { rm -f "$log"; echo "gate: the rerun failed without a failed test" >&2; return "$rc"; }
      flakes="$flakes"$'\n'"$(comm -23 <(printf '%s\n' "$failed") <(printf '%s\n' "$still"))"
      failed="$still"
    fi
    rm -f "$log"
    [ -n "$failed" ] || break
  done
  if [ -n "$failed" ]; then echo "gate: failed alone, a real failure: $(printf '%s' "$failed" | awk '{print $2}' | paste -sd' ' -)" >&2; return 1; fi
  while IFS= read -r line; do
    [ -n "$line" ] || continue
    name="${line#* }"
    if rerun_in_changed_file "$line" "$changed"; then bad="$bad $name"; fi
    names="$names $name"
  done <<< "$flakes"
  if [ -n "$bad" ]; then echo "gate: passes alone but sits in a file the branch changed, so it fails the gate:$bad" >&2; return 1; fi
  echo "gate: load flake:$names"
  mkdir -p "$HOME/.cache"
  printf '%s %s%s\n' "$(date '+%Y-%m-%d %H:%M')" "$(git symbolic-ref -q --short HEAD || echo detached)" "$names" >> "$HOME/.cache/k7-flakes.log"
  if [ -n "${K7_FLAKES_FILE:-}" ]; then printf '%s\n' "${names# }" >> "$K7_FLAKES_FILE"; fi
  return 0
}
