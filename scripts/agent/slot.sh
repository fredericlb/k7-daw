#!/usr/bin/env bash
# slot.sh — the fixed worktrees agents work in, `.claude/worktrees/slot-0` to `slot-7` (docs/agent-playbook.md).
# `create` | `take <n> <branch>` | `free <n>` | `status` (who holds each slot) | `free-for builder|reader|script`.
# Roles: 0-4 and 7 builders and beds, 5 readers' test runs, 6 script jobs. Run it from a folder that is not a slot.
# free-for: a slot is usable when clean, with no busy process, holding main or an idle branch (no process, nothing
# uncommitted). A shell whose only children are `sleep` and whose command is an `until`/`while` loop is idle, not busy.
# What a reader gets wrong: a build folder past K7_SLOT_LIMIT_GB (25) is moved aside at a take and deleted in the background.
set -euo pipefail
common="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --path-format=absolute --git-common-dir)"
clone="${common%/.git}"
slots="$clone/.claude/worktrees"
limit_gb="${K7_SLOT_LIMIT_GB:-25}"
SLOTS="${K7_SLOTS:-0 1 2 3 4 5 6 7}"

die() { echo "slot: $*" >&2; exit 1; }
dir_of() { case " $SLOTS " in *" $1 "*) echo "$slots/slot-$1" ;; *) die "a slot is one of: $SLOTS, not '$1'" ;; esac; }
role_of() { case "$1" in [0-4] | 7) echo "builder/bed" ;; 5) echo "reader" ;; 6) echo "script" ;; esac; }
kind_slots() { case "$1" in builder) echo "0 1 2 3 4 7" ;; reader) echo "5" ;; script) echo "6" ;; *) die "a kind is builder, reader or script, not '$1'" ;; esac; }
caches() { echo "$HOME/.cache/k7-slot-$1" "$HOME/.cache/k7-wincheck-slot-$1"; }
size_gb() { if [ -d "$1" ]; then echo $(($(du -sk "$1" | cut -f1) / 1048576)); else echo 0; fi; }

ready() {
  node "$1/scripts/agent/worktree-init.mjs" "$1" > /dev/null
}

# This script, the shells that started it and its pipeline (one process group) are left out: an agent runs it from inside its slot.
own_pids() {
  local p=$$ group
  while [ "${p:-0}" -gt 1 ]; do printf ' %s ' "$p"; p="$(ps -o ppid= -p "$p" | tr -d ' ')"; done
  group="$(ps -o pgid= -p $$ | tr -d ' ')"
  ps -ax -o pid=,pgid= | awk -v g="$group" '$2 == g {printf " %s ", $1}'
}

# Pids (not ours) whose working folder is inside the slot, one per line.
cwd_pids() {
  local mine; mine="$(own_pids)"
  (cd / && lsof -d cwd -Fpn 2> /dev/null) | awk -v d="$1" -v mine="$mine" '/^p/{p=substr($0,2)} /^n/{n=substr($0,2); if ((n == d || index(n, d "/") == 1) && index(mine, " " p " ") == 0) print p}' | sort -un
}

# An idle shell: a shell running a wait loop whose children are only `sleep` (or none).
is_idle_shell() {
  local c k
  case "$(ps -o comm= -p "$1" | tr -d ' ')" in *bash | *zsh | *sh | -*) ;; *) return 1 ;; esac
  ps -o command= -p "$1" | grep -Eq '(^|[ ;(])(until|while) .*(;|[ ])do sleep|(^|[ ;(])until ' || return 1
  for k in $(pgrep -P "$1" 2> /dev/null || true); do
    c="$(ps -o comm= -p "$k" | tr -d ' ')"
    case "${c##*/}" in sleep) ;; *) return 1 ;; esac
  done
}

# Prints "busy <pid>" or "idle <pid>" per process in the slot; a sleep under an idle shell is part of it.
classify() {
  local pids idle="" p pp
  pids="$(cwd_pids "$1")"
  for p in $pids; do is_idle_shell "$p" && idle="$idle $p "; done
  for p in $pids; do
    pp="$(ps -o ppid= -p "$p" | tr -d ' ')"
    if [[ "$idle" == *" $p "* ]]; then echo "idle $p"
    elif [[ "$idle" == *" $pp "* ]] && [ "$(ps -o comm= -p "$p" | tr -d ' ')" = sleep ]; then echo "idle $p"
    else echo "busy $p"; fi
  done
}

# A process whose working folder is in the slot (a gate, a bed, a build) would finish in another branch's tree.
idle_or_die() {
  local busy="" k p
  while read -r k p; do
    [ -n "$k" ] || continue
    if [ "$k" = idle ]; then
      case "$(ps -o comm= -p "$p" | tr -d ' ')" in *sleep) ;; *) echo "slot: ignoring idle shell $p" >&2 ;; esac
    else busy="$busy$p "; fi
  done <<< "$(classify "$1")"
  [ -z "$busy" ] || die "$(basename "$1") is in use by process(es) $busy: wait for them, or stop them first"
}

clean_or_die() {
  local dirty
  idle_or_die "$1"
  dirty="$(git -C "$1" status --porcelain --ignore-submodules=untracked)"
  [ -z "$dirty" ] || die "$(basename "$1") has uncommitted work, commit or discard it first:"$'\n'"$dirty"
}

sweep() {
  local c gb old
  for c in $(caches "$1"); do
    [ -d "$c" ] || continue
    gb="$(size_gb "$c")"
    if [ "$gb" -gt "$limit_gb" ]; then
      old="$c.old-$(date +%s)"
      mv "$c" "$old"
      nohup rm -rf "$old" > /dev/null 2>&1 &
      echo "slot: $c is ${gb} GB, past ${limit_gb}: moved to $old, deleted in the background; sccache refills it"
    fi
  done
}

case "${1:-status}" in
  create)
    for n in $SLOTS; do
      d="$slots/slot-$n"
      [ -d "$d" ] && continue
      git -C "$clone" worktree add -q --detach "$d" main
      ready "$d"
      echo "slot: slot-$n created at main"
    done
    ;;
  take)
    [ $# -eq 3 ] || die "usage: slot.sh take <n> <branch>"
    d="$(dir_of "$2")"
    [ -d "$d" ] || die "slot-$2 does not exist: run slot.sh create"
    clean_or_die "$d"
    if git -C "$clone" show-ref --verify -q "refs/heads/$3"; then
      git -C "$d" switch -q "$3"
    else
      git -C "$d" switch -q -c "$3" main
    fi
    ready "$d"
    sweep "$2"
    echo "slot: slot-$2 holds $3 at $(git -C "$d" log --oneline -1)"
    ;;
  free)
    [ $# -eq 2 ] || die "usage: slot.sh free <n>"
    d="$(dir_of "$2")"
    clean_or_die "$d"
    git -C "$d" switch -q --detach main
    echo "slot: slot-$2 free, at main"
    ;;
  status)
    for n in $SLOTS; do
      d="$slots/slot-$n"
      [ -d "$d" ] || { echo "slot-$n  ($(role_of "$n"))  missing"; continue; }
      b="$(git -C "$d" symbolic-ref -q --short HEAD || echo free)"
      dirty="$(git -C "$d" status --porcelain --ignore-submodules=untracked | wc -l | tr -d ' ')"
      read -r t w <<< "$(caches "$n")"
      echo "slot-$n  ($(role_of "$n"))  $b  ${dirty} uncommitted  last commit $(git -C "$d" log -1 --format=%cr 2> /dev/null || echo none)  build $(size_gb "$t")+$(size_gb "$w") GB"
      while read -r k p; do
        [ -n "$k" ] || continue
        echo "    $k pid $p  $(ps -o etime= -p "$p" | tr -d ' ')  $(ps -o command= -p "$p" | cut -c1-60)"
      done <<< "$(classify "$d")"
    done
    ;;
  free-for)
    [ $# -eq 2 ] || die "usage: slot.sh free-for builder|reader|script"
    ok=""
    for n in $(kind_slots "$2"); do
      d="$slots/slot-$n"
      [ -d "$d" ] || { echo "slot-$n: missing" >&2; continue; }
      busy="$(classify "$d" | awk '$1 == "busy" {printf "%s ", $2}')"
      dirty="$(git -C "$d" status --porcelain --ignore-submodules=untracked | wc -l | tr -d ' ')"
      b="$(git -C "$d" symbolic-ref -q --short HEAD || echo detached)"
      if [ -n "$busy" ]; then echo "slot-$n: busy, process(es) $busy" >&2
      elif [ "$dirty" -gt 0 ]; then echo "slot-$n: $dirty uncommitted file(s) on $b" >&2
      else
        [ -n "$ok" ] || ok="$n"
        echo "slot-$n: usable (holds $b)" >&2
      fi
    done
    [ -n "$ok" ] || exit 1
    echo "$ok"
    ;;
  *) die "usage: slot.sh create | take <n> <branch> | free <n> | status | free-for builder|reader|script" ;;
esac
