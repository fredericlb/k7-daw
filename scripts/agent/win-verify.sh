#!/usr/bin/env bash
# win-verify.sh — run one command on Windows, in the verify worktree C:\Users\flb\Src\k7-verify, from WSL.
# `scripts/agent/win-verify.sh <branch> <command...>`, e.g. `... my-branch npm run test --workspace=@studio/song`.
# It pushes the branch to `win`, checks it out detached there, then runs the command under Git Bash.
# Not from a session isolated in a worktree: the guard refuses Windows commands there (AGENTS.md).
set -euo pipefail

[ $# -ge 2 ] || { echo "usage: win-verify.sh <branch> <command...>" >&2; exit 2; }
branch=$1
shift
temp=/mnt/c/Users/flb/AppData/Local/Temp/k7-verify
# Git for Windows installs per machine (Program Files) or per user (AppData); the first that exists runs.
bash_exe=
for candidate in "/mnt/c/Program Files/Git/bin/bash.exe" /mnt/c/Users/flb/AppData/Local/Programs/Git/bin/bash.exe; do
  [ -x "$candidate" ] && { bash_exe=$candidate; break; }
done
[ -n "$bash_exe" ] || { echo "win-verify: no Git Bash found" >&2; exit 2; }
stamp=$(date +%Y%m%d-%H%M%S)
script=$temp/run-$stamp.sh
log=$temp/run-$stamp.log
mkdir -p "$temp"

git push -q win "$branch"

# The generated script runs under Git Bash; a lockfile change since the last run triggers `npm ci`.
{
  echo 'set -eo pipefail'
  echo 'main=/c/Users/flb/Src/k7; tree=/c/Users/flb/Src/k7-verify'
  printf 'branch=%q\n' "$branch"
  echo '[ -d "$tree" ] || git -C "$main" worktree add --detach C:/Users/flb/Src/k7-verify "$branch"'
  echo 'cd "$tree"'
  echo 'git checkout -q --detach "$branch"'
  echo 'lock=$(git hash-object package-lock.json)'
  echo 'if [ ! -d node_modules ] || [ "$(cat node_modules/.verify-lock 2>/dev/null)" != "$lock" ]; then'
  echo '  npm ci --no-audit --no-fund && echo "$lock" > node_modules/.verify-lock'
  echo 'fi'
  echo 'echo "== $(git log -1 --format="%h %s") =="'
  printf '%q ' "$@"
  echo
} > "$script"

echo "win-verify: $branch -> $* (log: $log)"
status=0
(cd "$temp" && "$bash_exe" -l "C:/Users/flb/AppData/Local/Temp/k7-verify/run-$stamp.sh") > "$log" 2>&1 || status=$?
tail -n 60 "$log"
echo "win-verify: exit $status (full log: $log)"
exit "$status"
