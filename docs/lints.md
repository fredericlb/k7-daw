# The five checks no test would catch

Beside `biome`, `tsc` and the test suites, this repo runs five checks of its own. Three block, two
report a debt. Each guards a property the ordinary tooling has no opinion about.

| script | npm | blocks? | refuses |
|---|---|---|---|
| `scripts/check-source-bytes.mjs` | `lint:bytes` | yes | a NUL byte in a tracked source file |
| `scripts/check-doc-links.mjs` | `lint:docs` | yes | a page or a source comment naming a repo file that is not there |
| `scripts/check-cargo-locks.mjs` | `lint:locks` | yes | a Rust workspace's `Cargo.lock` that no longer pins its manifests |
| `scripts/lint-rustfmt.sh` | `lint:rustfmt`, in `ci:local` | yes | a Rust workspace not as `cargo fmt --all` prints it; `gate.sh` checks only a branch's changed files, so the rest drifted unseen until a branch touching one file had to reformat it whole |
| `scripts/agent/generated-check.mjs` | `gate.sh`, when a source of a generated file changed | yes | a WebAssembly module (`k7Wasm.generated.ts`, `waveformWasm.generated.ts`), or the song crate's `types_generated.rs` not what its sources build (the tables are written first, since the wasm compiles them); it names the file and `npm run rebuild:generated` |
| `scripts/check-vectors.sh` | `ci:local` | yes | the k7x editor's and the host's committed vectors differing from what `packages/k7x/scripts/makeEditVectors.mts` and `packages/commands/scripts/makeHostVectors.mts` write, a corpus file edited under `demos/` or `k7-core/` included; it then replays the file service's frozen `tests/vectors/` (`k7-file-service`'s `vectors` and `rig::` tests: no generator, the TypeScript they were harvested from is leaving) and `song-edit`'s frozen vectors and goldens through the crate and the WebAssembly module (`npm run test:edit-replay -w @studio/k7-wasm`); it names the files and restores them with `git checkout HEAD -- <files>`, so it refuses to run while a merge is in progress, whose staged vectors that restore would discard |
| `scripts/check-comments.mjs` | `lint:comments` | no | long headers, multi-line body comments, French where English is expected |
| knip | `lint:dead` | no | dead and duplicated code on the audio axis |

`gate.sh` exports `K7_SETTINGS_DIR` to a scratch folder under `~/.cache` when unset, so the vitest runs' engine never writes the user's `Settings.xml`.

`lint:bytes` is the shortest and the least obvious: ripgrep skips a file containing a NUL byte
whole, so such a file stops existing for every sweep anyone runs afterwards.

## `lint:locks` — the tray's lock behind the backend's manifests

The tray (`apps/k7-tray/src-tauri`) links the backend's crates by path, so it is a second Rust
workspace with its own `Cargo.lock` over the same dependencies. A crate added to
`apps/k7/backend-rs` updates that workspace's lock, which every build there writes, and leaves the
tray's behind: the tray's next build resolves the new crate afresh, so a release ships versions no lock
chose, and the Windows checkout is left with a `Cargo.lock` nobody committed. `cargo metadata --locked`
resolves without compiling, so it needs neither tauri's system libraries nor a build: OneDev's
`lint and test` runs it in its `rust:1.97.1-alpine` step on every push, and `check:changed` runs
`lint:locks` when a branch touches a `Cargo.toml` or `Cargo.lock` of one of these workspaces or of the browser's crates below. Where cargo is
not on the PATH (the WSL clone) the script says so and passes, and OneDev is the check.

The same script holds a deny-list (`scripts/cargoLockDeny.mjs`) over every lock: a crate pinned there fails the check
whatever depends on it. An entry carries `enabled`, so a list can name a crate before its removal lands; the removal
flips it by changing one constant, and `scripts/check-cargo-locks.test.mjs` proves an enabled entry refuses a
lock that pins it (`scripts/fixtures/locks/`). `ci:local` runs it beside `lint:docs`.

The browser's crates, `packages/song-schema/rust`, `packages/k7-wasm/rust` and the compiler's `packages/song/rust`,
are held the same way, each its own workspace with its own lock (`packages/song-schema/docs/wasmBoundary.md`,
`packages/song/docs/rustCompiler.md`).

## `lint:docs` — a page that points at nothing

The method of this repo is that the WHY lives in the prose: a comment points at a `docs/` page, and
the page points back at the code it is about. A pointer to a file that has been renamed or deleted
sends the reader into a wall, and nothing else notices.

**Both directions are read**: the pages, and the source whose comments point at them — TypeScript
and JavaScript, C++, Kotlin, `.k7x`, GLSL, YAML, shell and Python (`SCANNED`). It reads what git
tracks and what it merely knows about, never what is ignored and never a build or install output
(`dist/`, `node_modules/`, `build/`…), and only paths that look like ours.

**What counts as "a path into this repo"** is anchored on the three TOP folders — `packages`,
`apps`, `engine` — and deliberately NOT on `docs/` or `scripts/`. Those two are written
relative to the zone a page is talking about: `architecture.md` says `docs/picture.md` meaning
k7's, and moteur's `protocol.md` says `scripts/two-sessions.ts` meaning its own. A check anchored
on them would report a hundred sentences that are perfectly clear to a reader.

The pattern's lookbehind is what keeps `[stems.md](../packages/k7-plugins/docs/turntables/stems.md)` from being
read as a reference to `docs/stems.md`: a match may open on `../`, never in the middle of a path. A
reference is resolved from the repo root first — how nearly every page writes one — then relative
to the page, which is how a `../../packages/song/src/…` written inside a package's own `docs/`
resolves. (Spelling a whole example path here would make this sentence a broken link itself.)

**Two lists soften it, and both are meant to stay short.** `ALLOW` covers paths a page may
legitimately name although this checkout has not got them: a page inside `engine/k7-runtime` writes
`engine/EditBuilder.cpp` relative to its own app — a path under one of `engine/`'s real folders is
still checked — and a path under `dist/` or a Rust `target/` names a build output a checkout has only once built.
A growing list there is the check going soft.

`TOMBSTONES` is the other one, and it is empty: a path listed there is exempted because the
sentence around it is about a file deliberately absent from the tree, and it is spelled out by hand
rather than detected, since a regex cannot be trusted to read that.

## `lint:comments` — where the prose lives

The three limits are in the root `AGENTS.md`: a file header of at most 10 lines, a body comment of
exactly one line, and English. It is non-blocking, like `lint:dead`: it reports a debt that predates
the rule and shrinks as files are touched. New code has no excuse.

**French is detected on FUNCTION WORDS, never on accents.** An accent test cannot
work: this codebase writes the names of its own things — the médiathèque, a dépôt, a chaîne —
inside perfectly good English sentences, so an accent test flags prose that is already fine and
never reaches zero. Function words are what a language actually is, and they also catch French that
happens to carry no accent.

**Quoted spans are dropped before the test.** The screens are in French, so an English comment
naming one quotes it — « Trouver des morceaux » is the button's name, not prose to translate. Only
the guillemet and double-quote forms are stripped, because the apostrophe is a French letter and
stripping `'…'` would eat the sentence around it.

One thing the rule forbids outright, and it is worth repeating here: **never turn a multi-line
comment into one line by joining its lines.** That produces an unreadable 500-character line and can
swallow a `biome-ignore` directive, which then silently stops working — a directive must start its
own comment. Condense to a single sentence and move the rest to a doc page.

## Why it scans blocks rather than matching line by line

A JSX comment's second line starts with PROSE, not with a marker:

```jsx
{/* The families moved from the side to the header, where k7 puts its views: the side is
    worth more as the current zone's tree than as three fixed buttons. */}
```

A line-by-line match sees that as one line and reports nothing, which leaves a whole class of
violations invisible. So `runs()` walks the file instead, opening on `/*`,
`/**` or JSX's `{/*` and closing on `*/`, and reports the whole run.

Two guards fall out of that. A run with no closing marker within 60 lines is a **template
literal**, not a comment — `gen-builtin-params.mjs` emits a generated file's own header inside one —
so the scan bails out rather than swallowing the rest of the file. And a **directive inside a run
of `//` lines SPLITS it**: a `biome-ignore` is not prose, and it may not share a line with any, so
the lines on either side are measured separately.

## A shebang looks like a body comment

Both `check-comments.mjs` and the ad-hoc counters skip a leading comment block as the file's HEADER.
A file starting with `#!/usr/bin/env node` has a first line that is neither blank nor a comment, so
the skip stops immediately and the header that follows is counted as body. The same happens to a
comment sitting under a file's imports. Neither is a real violation; the header rule is what
applies, and those headers are within it.

## `noUndeclaredVariables` — the one that only fails at runtime

Enabled in `biome.json`.

An identifier used but never imported **parses fine**. `node --check` passes, the file loads,
and it dies on whichever branch first reaches it — so a build script can be green for weeks and
throw `ReferenceError` on the one path that matters. It happened here twice in one afternoon:
a guard added to `spike-job.mjs` without its `existsSync` import, and seven `node:fs` calls left
dangling when `build-engine.mjs` was split into `stage.mjs`. Both were only found by running the
exact path, on the exact platform.

Two settings go with it:

- `javascript.globals` declares `Terminal` and `FitAddon`, which `scripts/console/index.html`
  gets from xterm's `<script>` tags and biome cannot see.
- `!**/.tmp-*` is excluded from `files.includes`. Scratch scripts at the repo root are not
  source, and linting them put 86 errors in front of the five that are real.

Biome skips JSON: much of it is written by generators, whose spacing is not biome's.
