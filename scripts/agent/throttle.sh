#!/usr/bin/env bash
# throttle.sh — `. throttle.sh "$@"` at a script's top re-execs it under macOS background QoS, so a bed's front, engine and Chromium get CPU first.
# `throttle.sh cmd...` runs a command that way (an ad-hoc cargo build, `ci:local`). A no-op off macOS, with K7_NO_BACKGROUND=1, or when already throttled.
# Never source it from bed.sh or what a bed spawns (docs/rust-builds.md, "An agent's builds").
k7_throttled() { [ "$(uname)" = Darwin ] && [ -z "${K7_NO_BACKGROUND:-}" ] && [ -z "${K7_THROTTLED:-}" ] && command -v taskpolicy > /dev/null 2>&1; }
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  if k7_throttled; then export K7_THROTTLED=1; exec taskpolicy -b "$@"; fi
  exec "$@"
fi
if k7_throttled; then export K7_THROTTLED=1; exec taskpolicy -b bash "$0" "$@"; fi
