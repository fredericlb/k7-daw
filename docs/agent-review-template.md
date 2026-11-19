# The standing review brief

What every reviewer is given, so the coordinating session writes only the three lines at the top. A reviewer
starts from this page and reads nothing else it was not pointed at.

**Per review, the coordinator names:** the branch, the milestone, the commits (`<from>..<to>`).

## What a reviewer reads

1. `AGENTS.md` and `docs/agent-playbook.md` (the self-check and the test rules in "Briefs and reviews").
2. The branch's brief (`docs/wasm-step*-brief.md`), the milestone's rows, and the branch's state note
   `/Users/mahrave/Src/k7/.claude/state/<branch>.md`.
3. Earlier reviews of the branch (`.claude/state/<branch>-review-*.md`): a lesson already written there is checked first.
4. The commits' diff, whole, and the files it touches around the diff.

## How it works

- **Read-only.** It edits no tracked file and commits nothing. It works in a detached worktree of its own,
  at the commit under review.
- **Builds with `--target-dir $HOME/.cache/k7-<worktree name>` before any `--`**, after `eval "$(node engine/scripts/sccache.mjs agent --export)"`:
  `cargo test --target-dir … -p <crate> -- <filter>`, never `cargo test -p <crate> -- --target-dir …`, which builds in the tree.
- **Checks the claims, never trusts them.** Each number the builder reported is reproduced; each "fails without" is seen by
  reverting the fix alone in the working tree (restoring it with a touch, not a copy) and running the test failing; a
  test that passes either way is a finding. A departure is judged against the brief, not against the builder's reason.
- Deletes only what it made, by exact path. Never pushes, merges or runs `npm install`.

## What it writes

`/Users/mahrave/Src/k7/.claude/state/<branch>-review-<milestone>.md`, in this outline and no other:

```
# Review of <branch> <milestone> (<commits>)
Reviewer, slot, commit checked out, what was reproduced (one line each).

## Blockers
What stops the next milestone: failure, file:line, fix. "None." when none.

## Fixes
Before the next commit point: each with file:line, the failure, the fix and the test that fails without it.
Each finding names the `docs/agent-self-check.md` item it falls under, or says "new class", so the list grows.

## Can wait
Each with file:line and why it can.

## Departures judged
Each departure the builder listed: kept or refused, in one line with the reason.

## Decisions for the user
Each with a recommendation and its reversible side.
```

## Its final report

At most 15 lines: the note's path, the blockers (one line each), the count of fixes and of can-wait items, the departures
refused, the decisions. The detail is in the note, which the builder reads at its next commit point.
