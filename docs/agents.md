# Agents in this repo

What an agent (Claude Code, mostly) is given to work here quickly, and why each piece is shaped the
way it is. The rules an agent follows are in `AGENTS.md`; this page is the tooling.

## A worktree is made ready by a script, not by memory

An agent works in a `.claude/worktrees/<name>` worktree of the main clone. A fresh one has no
`node_modules`, and nothing in it lints or tests: `npx` with no local tool DOWNLOADS one from the registry.
`node scripts/agent/worktree-init.mjs [path]` fixes that, in under a second:

- **`node_modules`** is a real folder of links to the clone's packages, EXCEPT the workspace links
  (`@studio/*`, the apps), which are recreated with their relative targets. A single
  link to the clone's `node_modules` would make every `@studio/*` import test the CLONE's code, so a
  change to one package would never be seen by the packages that import it.
- The tools' caches (`.vite`, `.cache`) are left out: a worktree has its own.

`paraglide/` needs nothing: every workspace's `pretest` and `pretypecheck` regenerate it.

## Hooks run it, and biome, without being asked

`scripts/agent/claude-settings.json` is the proposed `.claude/settings.json`. An agent may not
grant itself hooks or permissions, so a person installs it, by copying it there (`.gitignore`
lets that one file in). It holds:

- **The worktree init, on `PostToolUse` of `EnterWorktree` and on `SessionStart`.** There is no
  event for "a worktree was created": `WorktreeCreate` REPLACES git's creation and must print the
  path itself. The hook reads the worktree from its input's `cwd`; `$CLAUDE_PROJECT_DIR` stays the
  main checkout, so the script that runs is the main checkout's.
- **Biome on every file an agent writes** (`scripts/agent/biome-on-edit.mjs`). `AGENTS.md` asks
  every edited file to pass biome; a hook that exits 2 hands the report back while the file is
  still in front of the agent, instead of at commit time.
- **A read-only allowlist**: git queries, biome, vitest, tsc, the repo's lints and tests, and
  `tilt get`/`logs`. Nothing that writes outside the tree, pushes or deletes.

## `npm run check:changed` checks what the branch touched

`npm run lint`, `test` and `typecheck` walk the whole monorepo. `check:changed` diffs against the
merge-base with `main` (where a branch is merged), then runs biome on the changed files, the
two blocking lints (`lint:docs`, `lint:bytes`, under a second together), `lint:locks` when a Rust
manifest changed, and `typecheck` + `test`
of each workspace holding a changed file. `--dependents` adds every workspace that depends on one:
off by default, since a change to `song-schema` would otherwise test half the repo. `--dry` prints
the plan.

## An agent checks narrow while it works, wide at a milestone

| while | it runs | why |
|---|---|---|
| it edits | the one crate's or package's tests it touched: `cargo test -p <crate> --test all <file>::`, `npx vitest run <file>` | a workspace's tests relink every test binary; up to four agents share the Mac's cores |
| it commits | `npm run check:changed` | what the branch touched, and its blocking lints |
| a milestone ends | the touched crates' tests and clippy, and `npm run check:changed` | `npm run ci:local` is the merging session's, once before the merge: four agents each running it at every milestone loaded the Mac until timing tests failed |

## What the isolation guard refuses

A session isolated by EnterWorktree has its git commands checked to stay in the worktree. It refuses
what it cannot prove: `git -C`, `cd` into the clone, `-c protocol.file.allow`, a command whose text
contains `.git` (so `.gitignore` too), a long inline heredoc, a `for` loop running a tool with a loop
variable, a `$(...)` subshell inside `time` or a compound. Write the script to a file and run that;
split chained commands into separate calls.

## Generated files are rebuilt, never merged by hand

| choice | why |
|---|---|
| the browser's WebAssembly module (`k7Wasm.generated.ts`) merges with `merge=rebuilt`: git keeps one side, `npm run rebuild:generated` writes it from the merged sources | it is compiled output; a hand-merged one is neither side's sources, and the drift tests refuse it anyway; every change to the compiler's crates changes that module's `SOURCES` line |
| the `merge.rebuilt` driver is set in the clone's git config by `worktree-init.mjs` | a merge driver is a config entry, which git never reads from the repository |
