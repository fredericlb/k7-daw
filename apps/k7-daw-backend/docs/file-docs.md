# The compile's nine routes, answered natively (step 1o)

What a page loads to draw a project, answered by Rust from the song crate's shared sessions: the web and engine documents,
the errors, the graph, the hints, the surfaces, the I/O ports, the module catalogue and the base modules (`GET /project-doc`,
`/engine-doc`, `/project-errors`, `/graph` without `?at=`, `/hints`, `/surfaces`, `/io-ports`, `/module-catalogue`,
`/base-modules`). It stands on [file-native.md](file-native.md): the path model is 1a's.
The front's half is `packages/k7-front/src/routes/files/service/native/`
(`docs.rs`, `stack.rs`), the roots and the rig are `packages/k7-file-service` (`song_ask.rs`, `rig.rs`).

## What it decides

| choice | why |
|---|---|
| the compile's nine routes are one piece, `docs`; `/graph?at=` and the other compile-adjacent routes are separate pieces (1110, 1115) | these are the routes a reload pays for; the others have no view and nothing to gain |
| the sessions are shared: one `SongSessions` per account behind one lock, the front taking it only inside a blocking task, never across an await (1111, 1112, 1510) | one cache and one budget; a lock held across an await would wait on itself |
| the sessions' budget is 256 MiB on a server and a quarter of the embedded 192 MB heap (1506) | the sessions are native memory; at 192 MiB the capacity session is let go of after each answer and a reload compiles four times |
| the ask is made natively (`docs::asked_in`): the active project from the workspace's marker, the revision token from `Revisions`, `thawed` from the account's jobs in the set's order; the roots and the rig are made in Rust (1113 to 1117, 1505) | a route's ask must equal a verb's compile's key byte for byte, or the two never share a held answer; the marker is what every native reader names, so the ask cannot disagree with them |
| the ask is read on the blocking pool before the sessions' lock is taken, and carries the revision token (1114, 1116) | without a token a just-saved project is unsettled for two seconds, the moment its views are fetched |
| the native roots and rig are held to node's `songRoots` and `songRig` by frozen vectors, including a Windows-spelled home and code root (1512) | node's `path` on a Mac is not Windows' |
| the rig is `<studio>/preferences.k7x` read and never written; the empty rig is `{"channels":{"in":[],"out":[]}}` (1511) | the rig keys the session, so the defaults' shape must be the TypeScript's |
| `song_roots` spells every path through `k7_file_service::paths`, libraries by byte order, with no `cfg!` (1513, 1514) | the roots hash into the key: `c:/` against `C:/` is a second compile on Windows |
| a decline is said once per workspace and reason: no active project, a failed view, a roots or rig the native side refuses (1115, 1517) | a refusal names what the native side cannot answer |
| an answer is the session's own text sent without a copy; only the module catalogue is parsed, to map each `path` through `to_rel`, and weighs six times its text (1120, 1502, 1516) | a capacity document is 31 MB |
| a native compile runs on a thread of its own with the compiler's stack (`native/stack.rs`), answering through a oneshot, never on tokio's blocking pool (1522) | the pool's stacks are 2 MiB and an overflow is not a panic: it aborts the whole front for every account; the deepest input the k7x parser admits needs about 240 KiB in a debug build, so the pool would hold it, and the bound that matters is the compiler's own |
| the engine view's doc is cut at its own closing brace, found by a scan that skips strings and counts depth (`engine_doc_of`, 1119, 1523), never at the last `,"freezes":` | the freezes hold cuts of the project with user-written values, so a `freezes` key inside one moves a text search and the route would answer 200 with JSON that does not parse |
| `engine_doc_of` answers `None` for a failure or a text that does not end as `{"doc":…,"freezes":…}`, which is a decline (1518) | a slice that cannot be proved is a guess |
| a native answer carries `x-k7-compiler` and touches the renders it used (1121) | the header is the routes' contract |
| a session the front retires has its line said at once; a compile that panics outside the lock is counted and said (1509) | the host says a retired session's line at its next call, which may never come once the routes are native |

## What the derived views decide

| choice | why |
|---|---|
| every derived view (the compiled document, the graph, the errors, the hints, the catalogue, the I/O ports, the base modules, the web document) is keyed on the revision token, and a builder only reads | the token sees every change, so there is nothing else to remember to call; a builder that wrote would memoise its own side effect away |
| invalidating on this process's own writes alone is wrong | another replica's edit or an editor on the same disk leaves graph, document and error list agreeing with each other and contradicting the files on screen until the project is reopened |
| the picture's document is a cache of its own beside the song's, and `surfaces` is not cached | a song has a tempo and its picture a viewport, and both are warm at once; a project has a handful of small files and a surface is edited while someone looks at it |
| the graph of a folder other than the open project is built, never keyed | the token describes the active project and nothing else is honest to key on |
| `/project-doc` is the web view, `/engine-doc` the compiled document, and the picture is its own document with an empty one for a project with no picture lane | the desk plays a project it did not compile and needs the real document; "which did I get" must not be a runtime question, and no picture and a broken picture stay two answers |
| `/surfaces` is derived and always 200 | the parse and its diagnostics are the song's, and a second reader in the browser is a second answer to "is this surface valid" |
| a document a browser hands over is not validated field by field | it is the shape `/engine-doc` handed out and the engine decodes by key with defaults, so a mirror would refuse documents the engine would have played |
| a clip's extent when the document states none (`length: 0`) is the file's length measured here from the in-point, memoised on size and mtime, 0 when it cannot, never written into the song and never read from the sidecar | a browser cannot read a header on this machine; a copied `to:` goes stale when `clip process` repoints the source; the sidecar's fingerprint refreshes on its own schedule |
| the web clip carries `sourceName` only, `warped` meaning a map is at play with `warpOff` for the crossed icon, fades and `vel` only when stated, `event` straight through, and a mixer of level and pan only; a lane's takes use the same mapping | a browser must not resolve a path on the engine's machine; a document polled every two seconds must not carry explicit zeroes; a take must draw as the arrangement it was |
| an audition of a clip plays its window in file seconds, and an absent or empty `to` stays a length of 0 | a take trimmed to one bar of a five-minute pass must not play the whole file; the engine reads 0 as to the end |
| the resources list reads `clips/` through the compile's validated reader, leaves recorded takes out by their id pattern and lists a clip whose file is gone as `missing` | a file with no declaration is never a clip; the timestamp in the take's id is why nothing else matches; a panel that hid it would hide the one thing worth knowing |
| the graph's `stub` is a plugin slot with nothing, excepting a module whose type is its behaviour (`shaders`, `shapes`, `video_transitions`), `notSupported` a deliberate marker, and `unknownPlugin` an unknown type or an unscanned external | an empty `plugin:` on a shader is not a hole; the known types are kept by hand and a type missing from the list draws red with empty knobs while the module plays |
| an unknown format stays off the wire and an unscanned plugin goes to the engine, which warns and skips it | one is unloadable anywhere and the other is a condition of the machine, which the flag says up front |
| `offline` is known and has no `pluginId`; it draws its declared params and its panel is inert | an id would name an address no document contains: a tempo fader sending `set-plugin-param` for ever with no error |
| a red node has no `ui`, no `pluginId` and no native editor | only a plugin the engine can instantiate has a window; whether it loads is the engine's warning |
| a step's ports ride only past one and `mandatory` only when false, `category` comes from the module itself, `events` from its type folder, `defaults` only for the params a panel binds and `channel` as numbers only | absent says what the common case already says; a label is the rig's own naming and rewriting it would make the panel disagree with the file |
| the choice lists (`assetChoices`, `bases`, `shaders`) read the module's folder and its ancestors', answer undefined for none and never fail, and `rack` pads come from `resolvedRack` sorted by symbol | a thin local module extending a shared amp offers that amp's profiles; the pads are the sounds the engine will have and must not move because a key was retyped |
| a live input's tap is the input of the first module on its lane, as a second pass over the built steps, and a lane with no plugin gets none | the id must be one the engine really has |
| `trackSignals` is midi when every event names a sequence and audio otherwise, `trackClips` is sorted by position and matched to the compile by `start`, `trackAlternatives` rides past one, and `frozen` asks the compiler's own question | a lane is not typed; a clip at bar 9 may be authored before one at bar 3; the badge on the node and the silence in the engine must not disagree |
| every curve is drawn, bound or not, with points only for a drawn one, and a broken curve file reads as no curves | `curve new` would be a verb whose result you cannot see |
| a plugin's opaque state is kept as a base blob at `base/<name>.vstpreset`, and the snapshot answers 404 for no scanned plugin, 503 for no engine and 409 for nothing to keep | a patch of a commercial synth lives in chunk state that its parameter numbers do not hold; a snapshot with no window ever opened would be the factory defaults wearing a name |
| `/module-base` answers from the loaded module with its ancestors' bases nearest first, and the path is built here with both spellings | the bases live on the shared module a song reaches through a thin local one; the engine needs the absolute spelling and the browser the relative |
| `/preview` hands back the window it sent, and `length: 0` means to the end | a second resolution of `from` and `to` in the browser puts a mark where the sound is not |
| `/presets` lists only, over every scope with the scopes a new one may go to, and every call names a node, a scope and a preset name, never a path | applying, saving and updating are verbs run in the Worker, so the dropdown and the command bar share one path; the path is built server-side and refused if the name would leave its folder |

## Risks

| risk | answer |
|---|---|
