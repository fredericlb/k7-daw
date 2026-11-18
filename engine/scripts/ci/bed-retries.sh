#!/usr/bin/env bash
# bed-retries.sh — prints `--retries=1` and a reason line (stderr) when the Mac's 1-minute load is above K7_BED_RETRY_LOAD (default: the core count).
# A spec passing on retry is Playwright's "flaky" and exits 0; one failing twice still fails. Only the Mac bed asks; OneDev's beds keep the config's retries.
load=$(sysctl -n vm.loadavg 2> /dev/null | awk '{print $2}' | tr , .)
cores=$(sysctl -n hw.ncpu 2> /dev/null || echo 4)
limit=${K7_BED_RETRY_LOAD:-$cores}
if [ -n "$load" ] && awk -v l="$load" -v m="$limit" 'BEGIN { exit !(l > m) }'; then
  echo "bed: retries on (--retries=1): load average $load is above $limit, a spec that passes on retry is reported flaky" >&2
  echo "--retries=1"
fi
