# gate-lib.sh — sourced by gate.sh: the machine-wide gate lock and the load wait. Tested by gateLock.test.mjs.
# The lock is a directory with the holder's pid and branch; a dead pid makes it stale. K7_GATE_NO_LOCK=1 bypasses it.
# What a reader gets wrong: the quick checks run before the lock is taken, so only the long steps queue.

gate_lock_dir() { printf '%s' "${K7_GATE_LOCK_DIR:-$HOME/.cache/k7-gate.lock}"; }

# gate_lock_take <branch>: waits while a live process holds the lock, naming it every poll.
gate_lock_take() {
  local dir; dir="$(gate_lock_dir)"
  [ -z "${K7_GATE_NO_LOCK:-}" ] || return 0
  mkdir -p "$(dirname "$dir")"
  while ! mkdir "$dir" 2> /dev/null; do
    local pid branch
    pid="$(cat "$dir/pid" 2> /dev/null || true)" branch="$(cat "$dir/branch" 2> /dev/null || true)"
    if [ -n "$pid" ] && ! kill -0 "$pid" 2> /dev/null; then
      # An atomic rename lets only one waiter clear a stale lock.
      mv "$dir" "$dir.stale.$$" 2> /dev/null && rm -rf "$dir.stale.$$"
      continue
    fi
    echo "gate: waiting for the gate lock held by pid ${pid:-?} (${branch:-?})"
    sleep "${K7_GATE_LOCK_POLL:-10}"
  done
  printf '%s\n' "$$" > "$dir/pid"
  printf '%s\n' "$1" > "$dir/branch"
  K7_GATE_LOCK_HELD=1
}

# gate_lock_release: frees the lock only when this process holds it.
gate_lock_release() {
  [ -n "${K7_GATE_LOCK_HELD:-}" ] || return 0
  local dir; dir="$(gate_lock_dir)"
  if [ "$(cat "$dir/pid" 2> /dev/null)" = "$$" ]; then rm -rf "$dir"; fi
  K7_GATE_LOCK_HELD=""
}

# gate_timeout_scale: exports K7_TEST_TIMEOUT_SCALE=4 when the 1-minute load exceeds the CPU count at start (K7_LOAD fakes it); a value already set stays.
gate_timeout_scale() {
  [ -z "${K7_TEST_TIMEOUT_SCALE:-}" ] || return 0
  local cpus load
  cpus="$(getconf _NPROCESSORS_ONLN 2> /dev/null || sysctl -n hw.ncpu)"
  if [ -n "${K7_LOAD+x}" ]; then load="$K7_LOAD"
  else load="$({ sysctl -n vm.loadavg 2> /dev/null || cat /proc/loadavg 2> /dev/null; } | tr -d '{}' | awk '{ printf "%d", $1 }')"; fi
  if [ -n "$load" ] && [ "$load" -gt "$cpus" ]; then K7_TEST_TIMEOUT_SCALE=4; else K7_TEST_TIMEOUT_SCALE=1; fi
  export K7_TEST_TIMEOUT_SCALE
}
