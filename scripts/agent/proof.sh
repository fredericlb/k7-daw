#!/usr/bin/env bash
# proof.sh has|put <step> — a stamp that <step> passed on this exact tree, kept in ~/.cache/k7-proofs (K7_PROOFS) by tree hash.
# merge-branch.sh skips a step whose stamp matches the tree it is about to merge (docs/agent-playbook.md "Merging").
# What a reader gets wrong: `put` refuses a tree with uncommitted files, since the stamp names HEAD's tree, not the working files.
set -euo pipefail
dir="${K7_PROOFS:-$HOME/.cache/k7-proofs}"
verb="${1:-}" step="${2:-}"
[ -n "$step" ] || { echo "usage: proof.sh has|put <step>" >&2; exit 2; }
tree="$(git rev-parse 'HEAD^{tree}')"
case "$verb" in
  has) [ -e "$dir/$tree.$step" ] ;;
  put)
    dirty="$(git status --porcelain)"
    [ -z "$dirty" ] || exit 0
    mkdir -p "$dir" && : > "$dir/$tree.$step"
    ;;
  *) echo "usage: proof.sh has|put <step>" >&2; exit 2 ;;
esac
