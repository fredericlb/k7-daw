# The backend conformance suite

`specs/conformance/` is k7's backend contract, black box and without a browser: the specs run
against the Rust front (`apps/k7/backend-rs/docs/front.md`).

```
E2E_BASE_URL=http://127.0.0.1:8789 npm run e2e:conformance --workspace=@studio/e2e
CONFORMANCE_FILES_ROOT=C:/Users/me/.k7/dev-files …
```

On the Windows dev session the TypeScript backend answers on 8790 and the dev front on 8789; the file
service's specs pass against both.

The front's compiler is the native one, so every bed script runs the suite with `CONFORMANCE_SONG_NATIVE=on`: `serviceDocs` then drops the sentence of the compiler's error and keeps its status and the file it names, and
holds that the compile routes answer `x-k7-compiler` equal to `SOURCES`, which they answer in no other mode. `serviceGolden` runs in every mode, nothing held out: it names the file each broken fixture's error locates by this variable too.

`serviceGolden` and the engine: `CONFORMANCE_ENGINE_BIN` names the `k7-runtime` it renders with, else `<repo>/dist/k7-runtime/k7-runtime` (`.exe` on Windows). A binary that is not there fails the render tests with a sentence; nothing is skipped. `engine/scripts/ci/e2e-macos.sh` passes both variables.

Files run side by side, four workers by default (`CONFORMANCE_WORKERS`, or Playwright's `--workers`;
`--workers=1` is the serial run). A file's tests stay in order in one worker. The files that share a
state none of their names keeps apart are in a **lane**, a project of one worker in
`playwright.conformance.config.ts`, which runs beside the rest:

| lane | files | the state they share |
|---|---|---|
| `service` | the file service's specs but `serviceEngine` | one file service process per account, and the account's served project: two specs at once cut each other's calls (a 502 `file service: client error`) and read each other's open project. It is the longest lane, so it starts first |
| `engines` | `engines`, `engineJobs`, `serviceEngine`, `localEngine` | the engine registry and the session's engine binding: an engine one registers or claims is listed to the others |

Everything else writes only under names and accounts of its own, so it needs no lane.

### Candidates to make parallel later

A lane costs time: the `service` lane alone is most of a parallel run. Each could leave its lane once
what it shares is made its own:

| files | what kept them in a lane | what would free them |
|---|---|---|
| the `service` lane | one file service per account, and its served project | an account per file |

| family (phase 4b) | spec |
|---|---|
| an engine's slots and jobs: their refusals | `engineJobs.spec.ts` |

| family (phase 5a, `docs/rust-phase5-plan.md`) | spec |
|---|---|
| the hello and its ticket's signed shape, `/api/files`' gate (header, `?token=`, `~engine`), the identity headers replaced, a body streamed both ways, the internal seam's refusals | `filesHub.spec.ts` |
| the server's workspaces read file by file for a device to download: list, manifest, file, refusals, the door's token | `files_export.rs` (Rust, a server front seeded on disk) |
| `/api/ready`: loopback by Host, its volumes and the hub's starts | `ready.spec.ts` |

| family (phase 6.5) | spec |
|---|---|
| the built page, its fallback and the 404 page | `lastRoutes.spec.ts` |

| family (phase 5d, the file service, `docs/rust-phase5-plan.md`) | spec |
|---|---|
| the account's workspaces: Examples and its demos on disk, a make and its library pins, the listings, a copy between two, no sync route, the bench, a removal with its capture folder and marker | `serviceWorkspaces.spec.ts` |
| a project's life: each archetype against `scaffolds.json`, its names, an open and its marker, the default, its plugin needs, a rename, a delete into the recycle bin | `serviceProjects.spec.ts` |
| one file: read, write, a stale base, the paths refused, a dot folder, folders, renames, moves, the recycle bin, uploads and `/media` | `serviceFiles.spec.ts` |
| what an entry refuses and a library module's rename: the active project, `_modules`, the libraries, a case-only rename, a YAML name, refs repointed | `serviceEntries.spec.ts` |
| the undo journal: `journal.json` byte for byte, undo and redo on disk, `join` and `skip`, a graph pick's two lines as one step, a journal read back from disk, a process step's blobs | `serviceHistory.spec.ts` |
| sound: a waveform and its sidecar, the prewarm, an audition, resources, a mixdown, plugins, the engine routes, the logs | `serviceMedia.spec.ts` |
| the media verbs: a picture lane prepared, its cut written and read back, its sound extracted, whisper's transcript, a playlist's stems refused, the picture rendered on the server | `serviceMediaVerbs.spec.ts` |
| what a project compiles to: its documents, a verb's graph, a dsp decode error, an unresolved module | `serviceDocs.spec.ts` |
| what its builders read and make: libraries, catalogue, presets, playlists, modules and nodes made, importers, commands, preferences, the account's chains | `serviceModules.spec.ts` |
| the playlists and references listed, empty and after each file made, renamed and removed | `serviceListings.spec.ts` |
| the corpus as a smoke test through the running service: each archetype, demo and e2e fixture compiled, rendered by the engine and its sound judged against a table; the two broken fixtures refused naming their file | `serviceGolden.spec.ts` |
| the engine link as an engine hears it: an open held through the copy and a silent dry run, a write and a verb sent again, a take filed as a clip, a node frozen through the engine binary, a delete letting the session go, and with the actor no node service started for it | `serviceEngine.spec.ts` |

## The choices

| choice | why |
|---|---|
| Playwright's request API, in `e2e/` | the session is minted the way the functional suite mints it (`src/session.ts`, the instance's `SESSION_SECRET` from `env.yaml`), so a spec signs in as the e2e account without the identity provider |
| every spec states what a client sees, never how the backend does it | the same spec must hold for an implementation in another language |
| what proxies break gets a spec of its own: a body round-trip, the first frame of an event stream before its end, an upgrade refused with 404 | buffering, a dropped body and a swallowed upgrade are the faults a front introduces and a page only shows as a hang |
| writes are no-ops: the preferences and the AI settings are read and written back as they were, and a secret is only added where the instance had none, then removed | it runs against a live dev session, on the e2e account; a secret cannot be read back, so one the instance holds is never replaced |
| the auth specs mint and revoke the e2e account's tokens, and never log it out everywhere | a logout everywhere revokes a subject's every session, which must never be the e2e account's on a live session |
| the engine job routes are checked for their refusals only | a cut, a slot saved or a song sent changes the engine's machine |
| a spec's capture goes under a day of the 1990s, checked absent first, and removed by that day | no person recorded on that day, so a removal by day takes nobody's capture |
| an absolute path is refused as `/etc/passwd` everywhere and as `C:/Windows/win.ini` only when the server is Windows, which the file service's root says by its drive (`serverOnWindows` in `src/service.ts`) | on Linux a drive letter is a plain name inside the folder, a 404; the runner's own platform says nothing about the server's |
| the specs remove a capture's cached copy (`cache/capture-<hash>`) they caused | no route removes it, and a later capture filed at the same path would read a stale copy |
| files run in parallel, and a spec that cannot share a state with another runs in a lane rather than the whole suite running serially | the serial suite takes a quarter of an hour; a spec reaching an account-wide or instance-wide state is the exception, and naming the state it shares is what keeps the next spec out of it |
| the file hub's specs sign in as an account of their own, `conformance-files` (`src/sync.ts`), the one workspace removed by name until a listing no longer brings it back | a workspace of the e2e account is the functional suite's |
| `CONFORMANCE_FILES_ROOT` names the backend's `STUDIO_FILES_ROOT`; with it the public listing is read back exactly. Without it only the answers run | the other implementation loads those files as they are |
| the wire's own rules (the account key, `textHash`, a ticket's and a token's signature) are spelled again in the specs | a spec that called the code it checks would agree with any port of it |
| the file service's specs sign in as an account of their own, `conformance-service` (`src/service.ts`), make each workspace under a random `conformance-<hex>` name and remove it by that name until a listing no longer brings it back; the undo journals their projects made are removed by their folder names, computed from the service's own root | a journal outlives its workspace (the hub sweeps it after a month), and the account's other state is the suite's own |
| a scaffold is pinned file by file in `specs/conformance/scaffolds.json`: each file's sha256 with its random ids blanked, and what it adds outside its folder. `CONFORMANCE_WRITE_SCAFFOLDS=1` rewrites it from the instance | a port must write the same bytes, and a fixture of every scaffold's text would be a second copy of the archetypes to keep |
| a demo is copied out of Examples into a workspace of the suite's before it is opened; Examples itself is only read | an open repairs a demo's files in place, and the next run would find Examples no longer byte for byte the build's demos |
| the history spec writes `journal.json` and its `blobs/` by hand for a project the service never opened, then opens it | a service restarted, or another implementation, finds the journal on disk exactly so; no route restarts an account's service on a server, and a process step needs an engine to bounce |
| a read, an open, a make, a removal and a whole-file write are asked again when the hub answers `file service: …` with a 5xx; any other call is not | the dev session's hub stops every file service when a source of the Windows checkout changes, which other sessions' merges do several times an hour; asking those twice changes nothing |
| renders, waveforms and the audition need the engine binary, which the test bed has | a mixdown and a sidecar are files a port writes |
| the golden spec reads no committed fixture: a project must compile, render with exit 0 and, for the archetypes and demos, sound as its table row says; a fixture only counts toward the floors | a byte comparison pins the compiler that wrote the bytes, a render pins what the song reaches (`packages/song/docs/rustCompilerHost.md`, 900 to 910) |
| a film with sound is made by the runner's ffmpeg (`testsrc` and a sine, mpeg4 and AAC); without ffmpeg those specs skip. `CONFORMANCE_MEDIA_COPY` names a folder where the spec keeps the rendered picture and the extracted sound | no fixture film carries sound, and a committed one would be bytes to keep; two implementations' copies are compared byte for byte by whoever runs both |
| the engine link's spec plays the engine itself (`src/standInEngine.ts`): a WebSocket server answering `list-sessions`, listed by a heartbeat, behind the reference tunnel agent (`@studio/engine-tunnel/agent`), claimed by an account of its own (`conformance-engine`); its renders are the instance's engine binary | a second live engine beside a person's own replays Tracktion's saved audio device, which an ASIO driver serves to one process (`engine/k7-runtime/docs/audioDevice.md`); a take and a served project are the session's memory, which other specs read empty |
| the engine spec counts the node file services `/api/ready` reports before and after, and finds none more | a binding held on a node process would start one for every request naming an engine; the count is the one thing a client can read of the processes behind the hub |
| what needs a real engine's plugins, a second backend is not covered: auto-freeze (a VST and its scan), a process step kept on an engine, the prefetch, `.dawproject` and `.als` imports, the 1000-step ring, a conflict set aside and settled through the service | they need a VST scan, a second machine or a second replica writing the same value |
| what needs a second machine is not covered: the hub's 503 while a service starts or is held off after a crash | a server refuses the one, and the others need a file service that fails or a second backend |
| every bed runs the front as a device (`STUDIO_LOCAL_USER`), so the specs that edit a workspace (`service*`, `files*`, `workspace-ui`) run | a device's account is the only runtime (`e2e.md`, "The beds are devices") |
| `auth.spec.ts` (the session and the tokens), `cameras.spec.ts`, `settings.spec.ts`, the engines' (`engines.spec.ts`), the local engine's (`localEngine.spec.ts`) and the file hub's (`files*`, `sync*`, `ready`, `service*`) run against the front only | the backend has no copy of those routes left to compare with: their reference is the front and the committed vectors |
| the local engine's specs run against a machine's own backend only (`/api/local` answering), and send no command that changes the engine; the internal seam takes `CONFORMANCE_INTERNAL_SECRET` | a throwaway local-user backend is the test bed (`apps/k7/backend-rs/docs/local.md`); against the person's desktop app, a rescan or a slot would be theirs |
| on a phone, the forwarded ports are NOT 18789 (`adb forward tcp:28789 tcp:18789`) | 18789 on a computer running the desktop app is that desktop app's own backend: a forward onto it fails, and the specs then test the computer |
| on a phone, the release upload's refusal fails | a device (`STUDIO_LOCAL_USER`) turns every session into its local user and answers no app release. Compare a run with the previous one, not with a pass |

The backend's own HTTP tests (`apps/k7/backend/src/**/*.test.ts`) start the Express app in-process;
each route family's are rewritten here, as black-box specs, before that family is ported.
