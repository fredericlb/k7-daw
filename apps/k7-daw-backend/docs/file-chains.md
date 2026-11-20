# The account's chains, read natively (step 1p)

The account's chain lists (`GET /account/frozen-chains`, `/live-chains`, `/cold-chains`), the chain list of the download and the
hub's `cold-run`, answered by Rust. It stands on [file-native.md](file-native.md); the numbered
decisions are 1p-D1 to 1p-D30 of the work order, and each row lands here as the code that holds it does.

## What a cut's chain decides

| choice | why |
|---|---|
| a cut resolves its chain against the libraries, with the bench folder standing for the project, and an empty chain falls back to the engine's `stems` or `instrumental`, then the account's | a cut has no project, the engine's model and hardware decide the cut, and a gate that insisted on a chain turned away every cut the screen asked for |
| `want` names which fallback, and a chain that writes four stems asked for the instrumental is refused by name; a chain that bounces is refused by name | the instrumental is summed where the stems were cut so three stems never cross a network; a separator followed by a step has four inputs and no way to name one |
| a cut's chain is optional and an empty chain is not a missing one | the hub's internal route requires the user and the want and passes an empty chain to the fallback |
| a pass writes what the chain's module declares, and `instrumental` is the one output that is not a stem | summed while the parts are in memory, on the machine that cut them |

## The referee

`tests/vectors/chains.json` is frozen: it was harvested from the TypeScript's own functions over nine homes, and no
generator is kept. Per home: the chain walk, each template read and its steps' facts, the three lists, the cut's chain for each
`want` and chain, and `resolveChain` over a table of refs. A path under the scratch folder is written `<base>`.

- A home's file names are the same on every host: two names that differ only by Unicode normalisation are one file on a
  Mac, so the fixtures never hold such a pair.
- No fixture makes `resolveModule` throw, so the vectors hold no case of 1p-D22's whole-list failure; the native cut declines at 32 levels.
  The work order's self-extending steps are cycle-guarded in the TypeScript and do not throw, so they are not 1p-D22 cases.
- Each home carries the vocabulary itself (the refs and each folder, `@meta`'s spelled `<repo>/...`), so a module the Rust side
  keeps or drops with no chain naming it moves a row; a `windows` table holds the lexical fields over `C:/` paths (1p-D28).
- Every fixture reaches the case it names: `ports`, a side-by-side step second, an absent model with no `model_url`
  (`nourl`), and an empty one (`emptyurl`, refused by the schema, so the module leaves the vocabulary).

## Rows as built

| # | row |
|---|---|
| 1p-D2 | `templates/` holds `read_template` and `facts_of`; `Session::vocabulary` is one method over the cache's. A template's input is the chain file's own `ChainInput`; a step's module is the song crate's `Module` |
| 1p-D9 | A chain the reader refuses, a resolution with an error finding or a cut `extends` fold decline, and so do a file past 8 MiB or a chain past 50,000 steps; a folder chain's node that is a link out, a graph that is a folder and a preset that is a link out refuse |
| 1p-D11 | A step's `params` are numbers or strings; any other type (a list, a map, `null`, a bool) refuses the whole chain in both languages (`params-any`), and the Rust reader's refusal declines. The work order's "copied whatever their type" does not hold |
| 1p-D10 | The answers are `serde_json::Value`s in the literals' key order (`chains_vectors`): the account's chains sorted by ref, then the libraries' cold ones; a number is written as JavaScript writes it (`2`, not `2.0`) |
| 1p-D12 | `stemsChain` and `instrumentalChain` come from `rig::chain_settings`, beside the rig under the same file states: a string trimmed, empty or any other type is none (`chains_vectors`, homes 4 to 6) |
| 1p-D13 | A resolved model path is the vocabulary's own spelling (`/`, a drive's case kept); the sentences keep the literal `~/.k7/models` (`templates.rs`' `C:/` world, `chains_cold`) |
| 1p-D14 | `resolve_cold_run` is `resolveColdRun` rule for rule (`chains_run`, `chains_vectors`' runs); a chain holding a running step that is not offline declines |
| 1p-D24 | A folder chain with a missing node file is never cold, while the frozen list computes `cold` from the steps that read (`chains_cold`) |
| 1p-D25 | `find_by` takes a bare name's path match in any library before a last-segment match in the default one (`chains_run`) |
| 1p-D26 | `coldStepOf`'s sentence says `~/.k7/models` whatever `STUDIO_MODELS` names |

A refused template, a resolution that errs and a bound reached decline the whole request as `WouldThrow` and `TooLarge`: no `Declined` is added. The walk is its own, a second copy of 1e's (`chains/walk.rs`, typed by `readdir`, a link neither folder nor file; a graph file at a `chains/` root ends a library's walk as in 1e's) and its refs are the vectors' `dirs` (`chains_vectors`).

## The routes

| # | row |
|---|---|
| 1p-D7 | The bench is `<studio>/workspace/Banc/live.k7`, read and never made. A library cloned and no pins file declines (`pins`) |
| 1p-D8 | The vocabulary is built through the workspace's session in `SongSessions`, under the roots `song_roots` makes for the bench, on the compiler's stack (`stack::on_stack`); the lists share it and the walk is made once |
| 1p-D15 | The hub's cut is answered by `chains::cold_run_answer`, a decline a refusal in its sentence (`hub/answers.rs`) |
| 1p-D17 | No lock across an await: the sessions' lock is taken only inside the stack helper, the answering weight is taken before it |
| 1p-D30 | A list weighs a tree and the cut weighs `BASE_WEIGHT` (`answer_what`) |
| 1p-D16 | `POST /account/cold-chains/download` is answered where `chains` and `media` both answer (a download twice is two): the body parses and the owner passes as node's middleware does, the chain is found in the native cold view by ref (404 and 400 with the TypeScript's sentences), and each `(file, url)` starts in the account's `Media` registry in the chain's order, so a file named twice starts twice and answers one job twice while the first still runs (a job done at start, a bad name or a non-http url, gives two ids, as the TypeScript does). A `ref` that is no string, or `media` not answering, hands the request to QuickJS, counted and said once (`chains/download.rs`, `chains/tests_routes.rs`) |
