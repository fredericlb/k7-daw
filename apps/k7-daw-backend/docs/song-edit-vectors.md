# song-edit vectors

The Rust `song-edit` crate is `song-edit`: the TypeScript under `apps/k7-daw-frontend/packages/song-edit/src` and song's write side are stubs that call it.
Its proof is a frozen set of vectors, each a folder in and a folder out, in `apps/k7-daw-backend/tests/vectors/song-edit/` (one
file per module; `index.json` counts them and lists the exported functions no vector reaches) and the goldens in
`apps/k7-daw-wasm/song-edit/tests/goldens/`. They were harvested from the TypeScript implementation before the stubs replaced it, and
nothing regenerates them: no generator is kept, so the set cannot drift to follow the crate.

## Choices

- **Frozen, not regenerated.** The TypeScript the vectors came from is gone, so a generator would record the crate against itself.
  A change to the crate's behaviour that a vector contradicts is a deliberate edit of that vector, in its own commit.
- **A vector is a folder in, a folder out.** A call records the folder as text (`tree`), the files it changed (`changed`, `null` for a
  removal), the arguments, and the return value or the throw (`thrown`). A file that is not UTF-8 text, or over 64 KiB, is
  `{ "size": n }`: the replay writes that many zero bytes, as no `song-edit` function reads one.
- **A file is its own folder.** A path to one file, or to a file not written yet, records that file alone and `$DIR` is its folder;
  several paths record their common folder.
- **What cannot be replayed is kept apart.** A call with a function argument (`editModuleFile`'s callback) carries `replay: false`,
  as does a tree cut at 400 files; those are glue in TypeScript and are proved by the packages' own tests.
- **Texts are stored once.** `blobs` holds each file text by hash and `trees` each folder as path to hash, so a vector is a few lines.
- **Nothing varies between runs.** The folder is `$DIR`, the repository `$REPO`, another temp path `$TMP`. A random id is
  `$UUID<n>`, numbered by first appearance; a take's stamp (`YYMMDDTHHMMSS`) is `$STAMP`. A replay applies the same rewriting to
  what it produced before it compares.

## Who replays them

`apps/k7-daw-wasm/song-edit/tests/replay` and `replay_lane` replay them natively. `npm run test:edit-replay -w @studio/k7-wasm` has those
replays write the calls they screened, then runs each through `edit.call` over a temp folder, over the Vfs shims and in the built
Worker inside Chromium; `scripts/check-vectors.sh` and `gate.sh` run it (`apps/k7-daw-frontend/packages/k7-wasm/docs/edit.md`).
