# The commands package and the verbs

Every action a person can take on a song — from the command bar, a key, a button, a right-click — is a command line.
The catalogue of lines is `apps/k7-daw-wasm/verbs/commands.yaml`; the verbs that run them are the Rust
crate `apps/k7-daw-wasm/verbs`, which the page reaches through the backend's `POST /verb` (`docs/verbs-port.md`). The web
editor has no file operations of its own: a line runs over the project's files under the account's lock and lands its
writes as one journal step.

## Pieces

| File | Role |
|---|---|
| `apps/k7-daw-wasm/verbs/commands.yaml` | The **catalogue**: what you type, mapped to a feature SID, with its description, arguments and menus. Declarative. |
| `apps/k7-daw-wasm/verbs/src/palette.rs` | Builds the catalogue for the palette, served as `GET /verb/catalogue`: each command and each argument's class, provider and hint. |
| `apps/k7-daw-wasm/verbs/commands.json` | The crate's copy of the catalogue, written by `apps/k7-daw-wasm/verbs/scripts/catalogue.mjs`. |
| `apps/k7-daw-wasm/verbs/src/verbs/mod.rs` | The SID table: SID → handler, one file per family beside it. |
| `apps/k7-daw-wasm/verbs/src/complete/mod.rs` | The completion **providers** and their labels. |
| `apps/k7-daw-wasm/verbs/tests/support/mod.rs` | A fixture project in memory, a line run on it, and what it changed, compiled and undid. |

The work a verb writes lives in `apps/k7-daw-wasm/song-edit`; a render, a separation or a probe is a route of the front
the verb calls. A handler parses its arguments, calls them, and reports.

## A catalogue entry

```yaml
- command: "track order"
  sid: track.order
  readable:
    en: "Move a track to another position"
    fr: "Déplacer une piste à une autre position"
  help: "put a track at a position (e.g. track order gtr 2) — 1 is the top, past the end is the bottom"
  surfaces: [cli]
  menu: [track]
  args:
    [
      { name: track, complete: tracks },
      { name: position, hint: "1-based, counting lanes from the top" },
    ]
```

| Field | Meaning |
|---|---|
| `command` | The words typed. Dispatch picks the **longest** command that prefixes the line, so `track duplicate empty` is not shadowed by `track duplicate`. |
| `sid` | The stable feature id the handler is registered under. |
| `readable` | One imperative sentence per language (`en`, `fr`), no example, no full stop, 60 characters at most. The palette lists and fuzzy-matches on it. Both languages are required. |
| `help` | The fuller line with an example; shown on the help page. |
| `surfaces` | Only `cli` exists; the web dispatches under it too. Absent means everywhere. |
| `menu` | Which right-click menus may offer it: `track`, `group`, `node`, `event`, `param`, `curve`, `ruler`, `marker`, `assembly`, `warp`, `shot`, `pad`, `note`, `tap` (`MENU_ENTITIES`). Absent means every menu whose click answers the command's leading arguments; write it only to **narrow**. Unknown words are reported and dropped. |
| `confirm` | `true` makes the right-click ask before running (used by `track delete`). |
| `args[]` | Trailing arguments: `name`, and **either** `complete: <provider>` (values from the live project) **or** `values: [...]` (a closed set); `type: timecode \| number \| note` to shape-check a single-token argument before the handler runs; `hint`; `optional: true` (trailing only). An argument that swallows the rest of the line (a message, a name with spaces, JSON) carries no `type`. |

## Providers

The crate's providers answer `complete:` names from the project as its files say now: `tracks`, `nodes`, `modules`,
`baseModules`, `engineModules`, `categories`, `curves`, `sequenceFiles`, `audioFiles`, `clipTargets`, `chains`, `samples`,
`markerNames`, `moduleAlternatives`, `moduleEngines`, `assemblies`, `assemblyTemplates`, `playlists`, `libraries`,
`pictureLanes`, `surfaceTypes`. The page asks them through `POST /verb/complete`.

Each provider also has a **display class** (the provider table in `apps/k7-daw-wasm/verbs/src/complete/provider_info.rs`:
track, module, file, marker, choice…) and a default hint. The class colours the palette's field;
`class_hint` supplies the hint of scalar classes (`timecode` → "bar:beat:sixteenth — 5, 5:1, 5:1:1"). An unknown provider
in the YAML is reported at load rather than emptying the catalogue.

## Dispatch

The crate's `run_line`:

1. picks the spec by longest prefix; none is a refusal `unknown command`;
2. finds the handler in the SID table;
3. validates typed arguments positionally — a malformed timecode is refused with what it should look like;
4. refuses an event verb aimed at a picture lane's sound, except remove;
5. calls the handler with the raw argument string and the `Host`.

A handler **refuses** through `h.refuse(lines)` and returns: a refusal is not an error, nothing is written, and the web
shows it in amber. It reports through `h.log(lines)`.

## The host

`Host` (`apps/k7-daw-wasm/verbs/src/host.rs`) is everything a verb may touch: the files, the compiled `ProjectDoc`, `log` /
`refuse`, the effects `reload` and `applyProject`, and the front's routes. `h.call(route, args)` asks a route whose answer the
verb reads (a render, a peak, the plugin list, a probe…): with no answer yet the run ends pending, the backend asks the route
outside the lock and runs the line again with the answer. `h.ask(route, args)` asks one once the writes have landed (the
transport, a take arm, a plugin's window). The backend's half is
`apps/k7-daw-backend/packages/k7-front/src/routes/files/service/native/verb_ask.rs`
(`apps/k7-daw-backend/docs/file-changes.md`).

## A verb is never a plugin's

A feature that needs a verb adds a handler to the crate and a row to `commands.yaml`, as transcription's `track transcribe`
does (`apps/k7-daw-wasm/verbs/src/verbs/transcribe.rs`). A plugin brings what the page draws.

## Web-local commands

Some lines are answered by the page itself, not the verbs Worker, because what they do only exists there. They are listed in `apps/k7-daw-frontend/src/palette.ts` (`LOCAL_COMMANDS`, with `LOCAL_READABLE` and `LOCAL_ARGS`) and dispatched by `runLocalCommand` in `App.tsx`:

- **`ui …`** — view state only: never the project, never the transport. `apps/k7-daw-frontend/src/localCommands.ts` holds the exact names (`UI_EXACT`) and heads (`UI_HEADS`); the prefix is enforced both ways (`uiVerdict`). An unprefixed line that the server's catalogue names goes to the server even if it shares a word with a `ui` head (`curve set` is not `ui curve set`).
- **The bare transport verbs** (`BARE_TRANSPORT`: play, stop, toggle, rewind, panic) go straight down the engine socket for latency, falling through to the Worker when the socket is down or busy. `transport replay`, `locate`, `loop`, `record`, `step next/prev` are browser-only.
- **The selection verbs** (`selection copy/paste/delete/split`) read the clipboard and the rectangle, which are the browser's, and emit catalogue lines (`events add`, `events remove`, `event split`…).
- **Writes through a route rather than a verb**: `project undo/redo`, `project export`, `project import`, `project master fix`, the file tree's `fs file …` / `fs folder new`, `module create rack`, `sequence edit roll/tracker`.

`localCommands.test.ts` holds the palette's list against the dispatcher, so no command lists and then does nothing.

## Tests

`cargo test` in `apps/k7-daw-wasm/verbs`: each family runs its verbs over a fixture project (`tests/support/mod.rs`), reads
back the files, compiles the project and undoes the step; `tests/project.rs` fails on a catalogue verb with no handler.
The checks of the catalogue itself are the crate's `palette.rs` tests and `scripts/catalogue.mjs --check`.
