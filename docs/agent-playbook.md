# How agent sessions work here

What every Claude session working in this repository on the Mac follows: the agents, the merges, the local
builds, the Rust pitfalls and the checks. The tools' own pages hold the details: [agents.md](agents.md) (the
worktree tooling), [rust-builds.md](rust-builds.md) (the builds and the checks), [release.md](release.md)
(the CI).

## Where the work happens

| choice | why |
|---|---|
| every change in a worktree, never in the main checkout | the main checkout is the user's own branch and Tilt's dev session; a stray checkout or merge there lands in their work |
| a harness that runs the real engine binary gives it `K7_SETTINGS_DIR` (the engine's `--settings-dir`) and a scratch `STUDIO_HOME`, and does not count on a scratch `HOME` to keep `~/Library/Studio/Settings.xml` untouched (`apps/k7-daw-engine/k7-runtime/docs/settingsDir.md`) | JUCE on macOS finds its settings folder through the system, not `HOME`, so only the settings folder protects the user's `Settings.xml` and `~/.k7/exts.json`; `STUDIO_HOME` moves what k7 itself reads, and the bed's engine child gets both |
| the user's Tilt session and its ports (5223, 8789, 10350, 18789) and their real preferences are never touched | it is the session they work in |
| `df -h ~` checked before anything that builds or runs the bed | the disk has filled before; cargo target folders are 7 to 20 GB each |

## Agents

| choice | why |
|---|---|
| Opus designs a sub-step's brief and reviews each milestone; Sonnet builds and resolves merges with conflicts | most of the work is bounded against a spec; design, semantics and races are where a mistake stays quiet |
| a Haiku agent does work spelled out by file and line: files moved, references updated, a decision marked in a brief, a fix a review wrote out, test waits, script tweaks, doc marks, main merged into a branch when it merges clean or touches docs only, and the recurring check's gathering; a merge with conflicts in code stays a Sonnet agent's, and a long wait (a bed) is never a Haiku agent's | a fifth of the work needs no judgment, and a faster model turns it round sooner; a clean textual merge has still lost a registration here, so a conflicted one gets the stronger model; Haiku does the spelled-out jobs well and loses track of long waits |
| a Haiku agent's brief says to run `npm run rebuild:generated` and keep what it writes (main's `k7Wasm.generated.ts` taken back when no `apps/k7-daw-wasm/song` source changed), never to judge a file unbundled; what a Haiku agent says it skipped is checked before its work merges | its first job, done in 42 s, skipped the bundle by judging a bundled file unbundled, which only `main`'s `compileBundle` test would have caught |
| a builder's brief spans several milestones, and the builder waits on its own bed with one background command and a watch on the log's final line, never with polls; it reports at a merge point, a decision or the end, not after each milestone | relaunching a builder per milestone cost a long brief and a cold start each time |
| an agent that stops while its bed still waits in a queue is not relaunched to wait: the coordinator watches the bed's log for its last line itself and acts on the result | agents reported "still queued" again and again, each report a full turn |
| a Haiku agent runs a gate and triages its failures: a failure the gate reports after its own reruns alone is real and goes back to a Sonnet agent | the gate reruns a failed test alone itself; what it still fails on is not load |
| a Sonnet agent's change that is a list of exact edits (a review finding with its file, line and fix, a split by whole functions or tests, a comment made one line) goes to a Haiku agent, which applies, compiles, tests narrowly and shows the test failing with the fix reverted; a failure to compile or a test that does not fail goes back to Sonnet | the plan needs judgment, the edits do not |
| every Sonnet report ends with its mechanical follow-ups, each written as a Haiku job | the follow-ups were done by whoever came next, or not at all |
| one brief per sub-step (`docs/wasm-step*-brief.md`): files under 500 lines, milestones each with a gate, decisions numbered by sub-step, "Stop and ask", lessons from earlier reviews | a builder works from the page alone; a decision is the user's, the brief recommends one |
| a user decision is built on its recommended, reversible side, marked pending, and listed to the user in one batch; an irreversible one is left; a builder never takes a stop-and-ask case on itself as "non-interactive" | work does not stop on a question, and nothing is done that cannot be undone |
| approvals are recorded in `.claude/state/approvals-<date>.md`, and each builder marks the rows APPROVED in its brief at its next touch | a brief goes stale the moment the user rules, and the note is the one place a builder looks |
| a builder runs on to its next milestone after a green milestone gate, and stops only at the brief's stop points (a bed, a merge, a stop-and-ask, the end of the sub-step) or when its context runs low: then it stops at a clean commit and says so, and the coordinator relaunches it from its state note; only the coordinating session pushes, merges and holds the bed | relaunching a builder per milestone cost a long brief each time |
| a milestone is about 3 to 4 hours of work at most, and a brief splits a bigger one | builders ran out of context on milestones of 6 to 7 hours |
| before each commit a builder folds in `.claude/state/<branch>-review-<milestone>.md` of its earlier milestones: a blocker before it goes further, the fixes at the next commit point; a blocker found after the next milestone started is fixed first | a review lands while the builder builds the next milestone, and a builder that looks itself needs no finding relayed by hand |
| a builder's report is the short format: commits; the gate's total and its flakes; tests added, each with "fails without"; departures; decisions; blockers; next, at most about 25 lines, the detail in its state note, and each self-check item it could not satisfy | the coordinator read 60 to 150 lines per milestone to find these seven things |
| every brief repeats the build rules below: the `agent` mode exported, narrow checks, the Windows clippy, no push, no merge, no `npm install` | an agent knows only its brief |

## The order of a sub-step

| choice | why |
|---|---|
| the briefs are designed ahead, while earlier sub-steps build, and their decisions reach the user in one batch | a design took 1 to 2 hours on the critical path before anything could build |
| the critical path goes first: the chains that set the finish date get the next free place, and the longest remaining chain (today 1i) may go ahead of a shorter side branch | a side branch finished early moves no date |
| a branch that merged another open branch fixes their merge order, and a branch that others contain is never rebased | a rebase rewrites the commits the others hold |
| a branch may cherry-pick one self-contained commit from an open branch rather than wait for that branch's merge | the commit lands twice with the same content, which git merges clean |
| a milestone ends with `gate.sh --quick` and `npm run check:changed`; the full Rust `mac-checks.sh` runs once, at the merge, as a background command with a long limit, and the song crate's own clippy (`apps/k7-daw-wasm/song`, `--all-targets -D warnings`) beside it, since `mac-checks.sh` covers the backend workspace only | under five builders the milestone gate's Rust step hit the 30-minute limit and was rerun or skipped; a lint in the song crate's tests blocked a merge the backend checks had passed |
| `gate.sh --milestone` (the quick checks, `check:changed`, and `mac-checks.sh --narrow`, which checks only the backend crates touched since the branch's last milestone that passed (`~/.cache/k7-milestones/<branch>`) and those depending on them (`scripts/agent/affected-crates.mjs`), and runs a piece's twins and churn tests only when that piece or one needing it changed (`scripts/agent/affected-tests.mjs`; `K7_TESTS=all` forces them, and anything it does not recognise runs everything); the merge gate (`gate.sh`, every crate, adding `win-clippy.sh`, `android-check.sh` and `ios-check.sh`) runs once, before the merge | the full gate at every milestone cost 15 to 20 minutes each, and Android's 7 minutes and Windows' 3 only matter for what lands |
| `gate.sh --base <branch>` (or `K7_GATE_BASE`) gates a stacked branch against its parent, and the long steps (`check:changed`, `mac-checks`, the merge gate) run under one machine-wide lock, `~/.cache/k7-gate.lock` (pid and branch of the holder, stale when the pid is dead, `K7_GATE_NO_LOCK=1` bypasses; the quick checks stay outside); a failed test is rerun alone only once the 1-minute load is below the CPU count, waiting up to 3 minutes | a branch cut from an unmerged parent would otherwise re-check the parent's diff; two gates at once load each other into flakes, and a rerun on a loaded machine fails for the load it was meant to escape |
| the JS tests that starve under load (the frontend's, `k7-wasm`'s, `k7x`'s) take their `testTimeout` and their inline bounds through `scaled()` (`scripts/vitestScale.ts`), a factor read from `K7_TEST_TIMEOUT_SCALE` (default 1); `gate.sh` sets it to 4 when the 1-minute load exceeds the CPU count at start (`gate_timeout_scale`, `gate-lib.sh`) | those tests pass alone in seconds and fail only when the machine is loaded; a wider bound removes the false failure without dropping an assertion, and a quiet machine keeps the tight bound |
| an Opus reviewer reads milestone N while the builder builds N+1, and its findings go into N+1 | a review and its fix round at the end ran one after the other, an hour or more per sub-step |
| the bed runs beside the merge gate, from the branch's worktree, not after it; a Mac bed may run beside the builders | they share no process, and the bed alone is 20 to 40 minutes |
| a milestone's bed runs only its sub-step's own specs, the conformance suite filtered (`E2E_SUITE=conformance E2E_FILTER=…`, a few minutes); both suites whole run once, at the merge | with one bed at a time, two branches' full beds queued 20 to 40 minutes each behind one another |
| after every merge into `main`, each branch in flight takes `main` at once: a Haiku agent when it merges clean or in docs only, a Sonnet agent when code conflicts; a branch no worktree holds takes it without a checkout (`git merge-tree --write-tree main B`, then `commit-tree -p B -p main` and `update-ref`), its next builder compiling it | a branch that waited five hours took main in one heavy merge; small merges are seconds |
| a sub-step's critical chain may be split into two tracks on two branches (`step1x` and `step1x-io`, the I/O-free core and its I/O side behind fakes, made from it and merged back into it at each milestone's end) under a contract in `.claude/state/design-split-*.md`; the reviewer reads both | the two halves share no file until the routes join them, and the chain 1d → 1g → 1j → 1n is mostly one sub-step at a time |
| every builder keeps `/Users/mahrave/Src/k7/.claude/state/<branch>.md` (outside git, shared by every worktree): what is done with its commits, what is left, the review findings still open, the next step; it rewrites it at each commit, and a builder taking over reads it before anything else | a restart or an interrupt stops every agent, and a builder told nothing spent its first quarter re-reading the brief, the page and the log |
| a brief names, for each milestone, the TypeScript functions it ports and the Rust files it writes, with their paths | a builder spent most of a milestone finding its way; the reviews' file-and-line findings were fixed fastest |
| `npm run ci:local` runs on `main` after a batch of merges, by the coordinating session; a failure is fixed at once or the merge undone (`update-ref` back); it fails on drifted vectors | the merge gate's `check:changed` already tests every touched workspace; `ci:local` repeated it for 20 minutes per merge |

## The coordinating session

| choice | why |
|---|---|
| before saying what runs, `ListAgents`, each worktree's `git log` and `status`, and the processes (`cargo`, `nextest`, `e2e-macos.sh`) are looked at, not remembered | a report from memory has named agents that had already stopped |
| an agent's report is checked before it is passed on: a claim a reviewer doubts, a test that "passed alone", a number the next step depends on | a builder's report is model output, as confident when wrong |
| a decision the user must rule on is put to them in one message with a recommendation each, and work goes on beside it | the user rules faster on a batch, and nothing waits on one answer |
| an agent deletes only what it made in its own run, by its exact path, never by a pattern (`stage-*`, `*.log`); a bed's results live outside the session's scratchpad, in `~/.cache/k7-bed/<branch>-<purpose>/` | a cleanup by pattern deleted every bed folder in the scratchpad while three beds were writing into one, losing two hours of runs |
| `df -h ~` before a bed, a clean build or a fold; below 40 GB, the worktrees and `~/.cache/k7-*` folders of merged branches go first, never an unmerged branch's | the disk has run out three times; a merged branch's folders are what is safe to lose |
| after a restart of the machine, every worktree's commits since its known head and its uncommitted files are read, then each agent is relaunched told what was already done, and to judge what is uncommitted | a restart stops every agent and background command without a word; what was committed survives, and a builder told nothing redoes or undoes it |
| when git answers "You have not agreed to the Xcode license agreements", every agent stops and the user runs `sudo xcodebuild -license accept`; nobody works around it with `DEVELOPER_DIR` or the Command Line Tools on the `PATH` | an Xcode update leaves the licence unaccepted and stops git, `xcrun` and the linker; the workaround mixed two SDKs and broke Rust's links with "unknown architecture arm64e" |
| a builder never `cd`s into another worktree, and the coordinator never leaves its shell inside one: `git -C`, absolute paths and `( cd … )` subshells | a shell left inside a worktree made later commands run against the wrong branch |

## Briefs and reviews

| choice | why |
|---|---|
| a builder's brief names its worktree, its `--target-dir`, its gate mode, the files it may not create, and the known load flakes; it says to commit as it goes, to wait for its background work, and that its report is the short format (failures as failures, the detail and log paths in its state note) | an agent knows only its brief; a restart loses what was not committed |
| before `gate.sh --milestone` a builder runs `docs/agent-self-check.md` and lists in its short report each item it could not satisfy | the list is the classes the reviews keep finding (locks, races, disk order, Windows paths, quadratic fixtures, test sizing, one-line comments), each with a one-minute check; a builder that runs it first saves a fix round |
| a review finding spelled out by file, line and fix goes to a Haiku agent on a branch of its own (`step1x-fix`, from the builder's last milestone, in a worktree of its own), merged back by the builder at its next milestone's end; a finding that needs judgment stays the builder's | the builder goes on to the next milestone instead of carrying the fixes |
| a test is sized to what it proves: the smallest case that fails without the fix, with sizes a person or a client can produce (a project, a burst, a folder of the size the suites or the product make), or a hostile input the brief names with its bound; it runs in seconds in a debug build under load; a scale or capacity case counts operations (n against 4n) or is an `#[ignore]` measurement run by hand, never minutes of wall time in the gate; a budget or a limit is shrunk for the test through a `#[cfg(test)]` hook rather than flooded | a test of a weight rule made 340 real saves and took over six minutes of every milestone check; a time ratio under load failed and passed at random; neither proved more than a 40-save case does |
| every fix comes with the test that fails without it, shown by reverting the fix alone in the working tree and restoring it with a touch, not a copy | a test written beside a fix often passes either way; a restored file with an old mtime leaves cargo a stale binary |
| a reviewer is launched from `docs/agent-review-template.md`, the coordinator naming only the branch, the milestone and the commits; it reads only, checks the claims, ranks its findings (blocker, fix, can wait) with file, line, failure and fix, tags each with a self-check item or "new class", judges each departure the builder listed, and names the user's decisions | its note is the next brief's input; a review that edits mixes two authors, and a brief written per review repeated the same page |
| a review whose builder starts later is saved as `/Users/mahrave/Src/k7/.claude/state/<branch>-review-<milestone>.md`, beside the branch's state note, and the next builder's brief points at it | a session's scratchpad dies with the session, and another session could not find it |
| a failure is a branch's only when the same test passes on `main` run the same way, on the same OS: a bed is judged as an A/B (main's run, the branch's run, `scripts/agent/sameFailures.mjs`), a Rust test that fails is run on `main` in a throwaway worktree (`git worktree add --detach ~/.cache/k7-tmp-<name> main`, removed after) before anyone fixes it | failures blamed on a branch were main's own (stale expectations, Linux-only bugs) or load, and a fix aimed at the wrong side cost a round |
| a test that waits for something waits on an observable condition (a trace gone quiet, a frame count reached, a revision that stopped moving) under a generous bound, never on a fixed window; a comparison follows the protocol's own order guarantees, by set or by key where it orders nothing | the engine twins failed 2 to 4 of 69 per run at a load of 20 to 30 on a 1 s window, a different set each time |
| a test that fails under load and passes alone is rerun alone before it is called failed, reported all the same, and fixed when it recurs; the recurring ones today are `k7-hub`'s instance tests (10 s waits on child processes), `tests_entries::the_plan_shadow_never_costs_the_user_the_answer` and `tests_entries_end::a_write_whose_client_left_still_tells_the_bundle` | a flake that is only rerun costs every later gate a rerun |

## Merging

| choice | why |
|---|---|
| main merged into the branch first, a generated file rebuilt (`npm run rebuild:generated`), then the merge gate (`gate.sh`) green | a branch is checked as it will land |
| every Rust workspace stays as `cargo fmt --all` prints it (`npm run lint:rustfmt`, in `ci:local`); a branch that merges main over the formatting commit takes its own side of a conflict in a `.rs` file, then runs `cargo fmt` on that file | an edit to an unformatted file failed the gate's rustfmt step, so builders worked around files instead of fixing them |
| a branch that adds a test file to a crate whose tests are one binary (`tests/all/`) moves it there and lists it in `main.rs` when it merges main | a new top-level `tests/<name>.rs` is one more binary to link at every run (`docs/rust-builds.md`) |
| a branch is merged by hand from outside its worktree: `main` merged into the branch (a conflict is aborted, a generated file rebuilt with `npm run rebuild:generated`), `gate.sh` green, then by plumbing, never in the main checkout, `test "$(git rev-list --count BR..main)" = 0`, `commit-tree 'BR^{tree}' -p main -p BR`, `update-ref refs/heads/main NEW OLD`, the ancestor check (`git merge-base --is-ancestor BR main`) and `git branch -D`, each step chained with `&&` | the main checkout holds the user's branch, `update-ref` with the old value fails when main moved, and a step chained with `;` merged a branch whose gate had failed |
| a builder's last step before reporting is main merged into its branch in its worktree and committed when clean, the generated files rebuilt, then `gate.sh` | a conflict or a stale generated file is fixed by the builder, who still has the context, not found by the coordinator at the merge |
| a tooling or test-only branch merges after `gate.sh --quick`; a sub-step's branch merges after the full merge gate (`gate.sh`, no flag) and its final bed, judged against main's bed with `K7_BED_REF`; an engine (C++) change gets the full merge gate and the phone's full APK (`gradlew :app:assembleE2e`, `apps/k7-daw-engine/k7-runtime/docs/android.md`), since `android-check.sh` never compiles the engine | the quick gate proves what a tooling change touches; a sub-step's behaviour shows only in the full gate and the bed, and the engine's phone build only in the APK |
| `main` is pushed only when the user asks, or by a scheduled task; never force-pushed | a push publishes to the public repository: the user decides when, and the nightly run pushes only a main whose checks are green |
| a branch whose history carries what main must not (a build folder committed by accident, `TEMP` diagnostic commits) lands as ONE squashed commit: a `<branch>-squash` branch from main, `git merge --squash <branch>`, one commit with the cut's summary, then the merge above on it; the branch's own history is never rewritten | 113 MB of a cargo `target/` folder reached a branch's history; a plain merge would carry it into main and every push; `target/` is ignored repo-wide since |
| a branch ready while its builder goes on (the next milestone in the same worktree) merges through a pointer branch at the ready commit, checked out in a worktree of its own | merging the builder's own branch would merge main into its working tree under it |
| a bed or a gate left running for a branch already merged is stopped by the coordinator, by its pid; an agent's own stop is refused by the permission classifier | a stale bed held the Mac's one bed lane while live branches queued behind it |
| when `main` moved since the branch's merge gate and its new commits touch none of the branch's files nor their neighbours, `gate.sh --quick` and `npm run check:changed` are the whole re-check | re-running a branch's own suites for commits nowhere near it buys nothing |
| once merged, the branch's worktree and its `~/.cache` folders are removed | old folders are the disk |

## Local builds

| choice | why |
|---|---|
| `eval "$(node apps/k7-daw-engine/scripts/sccache.mjs agent --export)"` before cargo: four jobs, eight nextest threads, line tables only; `--export` makes `eval` set environment variables, which a bare `KEY=VALUE` does not | four agents at cargo's default loaded 15 cores to 72; the cap is fixed, since sccache keys on every `CARGO_` variable |
| one target folder per worktree, `--target-dir $HOME/.cache/k7-<worktree name>`, used by the agent's builds and by `mac-checks.sh` alike, `win-clippy.sh` keeping its own beside it; never `CARGO_TARGET_DIR` | sccache misses on that variable; a folder per worktree keeps agents off each other's lock; a check in a folder of its own compiled the whole workspace cold again and filled the disk 10 to 14 GB a worktree |
| `--target-dir` goes before any `--` (`cargo test --target-dir D -p c -- filter`); `gate.sh --quick` fails when `apps/k7-daw-backend/target` exists in its worktree | after `--`, the flag reaches the test binary and cargo builds in the tree: such folders sat in every slot, 7 to 20 GB each |
| while working, only the touched crate's or file's tests (`cargo test -p <crate> --test all <file>::`, one vitest file); at a milestone's end `gate.sh --milestone`; `npm run ci:local` only on `main`, after merges | a workspace run relinks every test binary and costs everyone's cores |
| a builder runs the bed through `scripts/agent/bed.sh`, never `e2e-macos.sh`: it waits for the Mac's bed slot, one bed at a time, and its results land in `~/.cache/k7-bed/<worktree>/mac` (`e2e/docs/e2e.md`) | a bed run takes 10 to 30 minutes and a second one beside four agents loaded the Mac until timed tests failed |
| the bed runs on the Mac only; Linux and Windows have no local bed, only GitHub's manual runs, which gate nothing (`docs/e2e-runners.md`) | k7 is a Mac app for now, and a failure only those platforms show is not looked for |
| a bed runs one mode, every piece native: `bed.sh` and the bed scripts name no switch | every host runs the whole native front, and a bed in another mode proves nothing of what ships |
| a bed's log has a name of its own (`~/.cache/k7-bed/<worktree>/…` or a log named after the branch), never a shared scratchpad name | two agents writing `bed.log` overwrote each other's result |
| the Mac's own bed: a full bed (`E2E_SUITE=both`) holds both slots, one suite per slot; `E2E_BED="2"` sends a one-suite run from another worktree to slot 2, never two runs from one tree; its suites run while this user's compilers yield the cores | a run stops every process of its own tree, so a second run from the same worktree kills the first |
| `gate.sh`, `mac-checks.sh` and `ci:local` run at background priority (`scripts/agent/throttle.sh`, macOS `taskpolicy -b`), and an ad-hoc cargo command too (`bash scripts/agent/throttle.sh cargo …`); a bed and what it spawns never do; a Mac bed started at a 1-minute load above the core count retries a failed spec once (`apps/k7-daw-engine/scripts/ci/bed-retries.sh`), a pass on retry counting as flaky | builders compiling at full priority starved a bed's front, engine and browser until timed specs failed; a spec that fails twice still fails the bed |
| the tests that make folders of 20,000 files run one at a time, through nextest's `file-churn` group (`apps/k7-daw-backend/.config/nextest.toml`, `docs/rust-builds.md`) | five at once woke the Mac's file-event and security agents until the load passed 60 |
| a branch's bed compared with main's by `scripts/agent/sameFailures.mjs`; the native counters (the engine link, the jobs, media, the shares, the runtime line, the shed) judged by `scripts/agent/bedCounters.mjs` at the end of every bed script | a failure main shares is not the branch's, and a run whose counters stayed low exercised too little to read green |

## Rust that holds on every platform

| choice | why |
|---|---|
| `bash scripts/agent/win-clippy.sh` before a merge that touches Rust, while Windows is set aside too | a `cfg(unix)` import or a Windows-only API fails only on Windows; a minute here keeps that build compiling until it is checked again |
| code that spells paths runs its tests on the Mac under the Windows rule too (`PathRule::Windows`, or `PathRule::HOST` forced while testing) | `Roots` and its rule default to the host's, so a test that assumes Posix passes here and fails there |
| tests spell a canonical folder without `\\?\` and with `/`, as the host does | Windows' `canonicalize` adds `\\?\`, which the Windows rule reads as a path its disk refuses |
| a link to a file made with `symlink_file`, to a folder with `symlink_dir`, and no POSIX mode without a `cfg` | Windows types its links and has no modes |
| an expected error code is per platform where Windows answers otherwise (a path below a file is `NotFound` there, `ENOTDIR` elsewhere), and only where the TypeScript answers the same way | the shadow compares the two implementations on the same platform |
| a time bound in a test is generous (30 s under musl) or replaced by a count | Linux's container and a loaded Mac run several times slower |
| a path is spelled one way at every boundary: real paths reach `song-edit` through `winpath::to_posix`/`from_posix` at one edge (`edits/edge.rs`), roots are keyed by their resolved form (`resolve_all`), and a canonical `\\?\C:\…` is never compared with `C:/…`; what crosses to the engine machine is a plain drive path | every Windows bug found in the port was two spellings of one folder: refused reads, duplicate relay batches, empty module trees, a take filed nowhere |
| a file is synced through a handle opened for writing, and a folder's sync is `cfg(unix)` | Windows refuses to flush a read-only handle, and has no folder sync |
| a test spells its temporary roots with `/` and without `\\?\`, and a Windows-only test of a fixed path rule runs on every OS through the rule, not through `cfg(windows)`; Windows' own tests wait for a Windows machine | the Mac never runs a `cfg(windows)` test, and two rounds of Windows fixes passed every Mac check |

## Where a branch is checked

A branch's checks and its beds run on the Mac; Windows' Rust tests wait for a Windows machine. One OS per
milestone is enough: a platform-only failure goes to a job of its own.

`bash scripts/agent/gate.sh` runs the checks below in order, cheapest first, and stops at the first failure: rustfmt on
the changed Rust, `lint:docs`, `lint:bytes` and biome on the changed files take a few seconds, so a formatting or link slip
never costs a 15-minute run. `--quick` adds the generated-files check and, when a `.rs` file changed, clippy and the
Windows clippy on the affected crates; `--milestone` goes on to `mac-checks.sh`, and `--ci-local` adds
`ci:local`. It prints each step's seconds and the total when it ends, failure included: a report quotes them, so a speed
change is judged on numbers.

| check | runs | when |
|---|---|---|
| `npm run ci:local` | lint, typecheck, the workspaces' tests, and a failure on drifted vectors | on `main` after a batch of merges, by the coordinating session |
| `bash scripts/agent/mac-checks.sh` | clippy and the tests of the backend and the browser crates, the song crate's wasm build; an area the branch did not touch since `main` is skipped; a piece's slow tests run only when that piece changed (`scripts/agent/affected-tests.mjs`), the merge gate and `ci:local` running everything | before every merge touching Rust |
| `bash scripts/agent/android-check.sh` | the Rust front built for the phone, the bundled backend, the app's Kotlin, with Gradle's configuration cache; not the C++ engine; skipped when nothing the phone's build reads changed since main (`--all` runs it) | in the merge gate |
| `bash scripts/agent/ios-check.sh` | `k7-embed` built as a static library for `aarch64-apple-ios` and `aarch64-apple-ios-sim` in `~/.cache/k7-ios-check`, then `xcodebuild` for the simulator against the frameworks `stage.mjs --sim` staged (it stops if they are missing); not the C++ engine; skipped when nothing the app reads changed since main (`--all` runs it) | in the merge gate, not `--quick` |
| the e2e bed (`scripts/agent/bed.sh`) | the web and conformance suites, every piece native, the native counters judged, compared with main's by `sameFailures.mjs` | at a piece's bed milestones, beside the merge gate |

| choice | why |
|---|---|
| Windows' Rust tests run for a branch that touches paths, files or processes, and once after a batch of such merges | the Windows tests found real bugs the Mac's checks never show (path spellings, read-only syncs, sharing violations) |
| every `cargo test` of the checks runs with `--no-fail-fast` | a failing test binary would hide the next failure |
| `--quick` runs clippy (`-D warnings`, all targets) and `win-clippy.sh --narrow` on the crates `affected-crates.mjs` names, whenever a `.rs` file changed | `clippy::redundant_async_block`, an import unused only on Windows and a struct literal broken by a merge were each found by the long gate, which then ran again |
| `--quick` runs `generated-check.mjs` when a source of a generated file changed: the WebAssembly modules by their source hashes; a stale file is named with `npm run rebuild:generated` | a merge or an edit left a generated file stale and the drift test found it at the end of a gate |
| a nextest run that fails reruns its failed tests alone (`--test-threads 1`), up to twice each (`rerun-failed.sh`); one that passes, in a file the branch did not change, is a load flake: printed as `gate: load flake: <names>`, appended to `~/.cache/k7-flakes.log` and listed in the gate's summary, and the gate goes on; one that fails alone, or sits in a changed file, fails the gate | six tests failed only under load in one day and each failure meant a whole gate run again; a test in a changed file is the branch's to explain |
| `nextest.toml` gives the known load flakes `retries = 2`, so a pass on retry shows as flaky in the run itself; an entry leaves the list when the `flaky-waits` job fixes its test | the retry is a bridge, not a cure: the list is the debt |
| a builder runs `gate.sh --milestone` and the merge gate as ONE background command and waits for its notification, never in a foreground call nor with repeated short polls | an agent's own 10-minute command limit killed gates that take longer, and the polls spent turns on nothing |
| between fixes a builder runs `gate.sh --quick`; `--milestone` runs once per milestone, and again only after a real fix of what it found | a milestone gate rerun after every small fix cost 10 to 20 minutes each time, for what `--quick` catches in under a minute |
| a test's time bound is generous (15 s, 60 s under musl) or replaced by a count | a loaded Mac runs several times slower |

## Starting a session

A session that takes over reads this page, then `/Users/mahrave/Src/k7/.claude/state/` (a note per branch, the reviews
saved for builders still to start, the approvals), then checks what runs. An agent that stopped is relaunched from its
note.
