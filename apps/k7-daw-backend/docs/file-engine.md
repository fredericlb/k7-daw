# The engine link in native Rust

The account's link to its engine, held by the front.
This page holds the choices and what forced them; the numbered rows (2200 to 2299) say where each is held.

## Where the line runs

| choice | why |
|---|---|
| the front owns the sockets, the binding, the sessions, the load and scan gates, the copy to the engine's machine, the reconcile and the poll | what waits on the engine or on a timer must not wait behind a verb |
| every frame and every document of a session leaves in the order it was asked | a verb's documents and its transport reach the engine in the verb's order; a compile on another thread would otherwise let a transport overtake the document it follows |
| the crate's part is sans-I/O: each machine (a session's load, its scan wait, its copy, its reconcile) takes events and a clock and answers effects; the front's part runs the sockets, the timers and the jobs channel that carry them out | the TypeScript's timers become a clock a test turns, so the 2-minute apply bound and the 10-minute scan bound are tested in microseconds |

## What the link and its load decide

| choice | why |
|---|---|
| a project is playable when the gate lets go: `scanning`, `copying`, `applying`, `warming`, one phase at a time, held in front of the editor | left behind the editor, the first play pays for the copy, the build and every plugin's first block in the worst way: a Stop that answers late |
| a load starts only for a project new to its session or new to its engine (told by address), never on a second `POST /open`, a save or a reconnect to the same engine | an engine that holds the document warm needs no dry run, and one on each edit would make every edit a stall |
| no engine bound is `copying`, never ready | the page's own socket says nothing about the service's binding; answered ready, Play reaches an engine the project has not reached |
| a load whose project names an external plugin waits for the engine's scan, and only a load does | the catalogue before the scan compiles the node away; a save under an open project is never held, so a rescan cannot take Play from a song already playing |
| the scan wait always ends: an engine silent for 3 s or not knowing the verb is not asked again, no progress for a minute or ten minutes in all gives up and says so for 30 s, a closed link is judged afresh on the next | a gate that waits for ever is a project nobody can open |
| the dry run sends the master at -100 dB (exact zero, no rebuild) for half a second, stops, rewinds, keeps silent for the reverb's tail, then sends the latest document at its own level | plugins have produced their first block before a person presses Play |
| the dry run turns record off whatever the link read and puts back only what the link read | a record roll records: a count-in, a take, a camera opened in the browser; the stop of a record roll disarms the person's own press |
| every flush of a project switches record off and notes it on the link | record is the engine session's, and one session plays every project of an account; the next song would open with record lit and nothing armed |
| a replaced dry run stops at once, ahead of the flush and the document of the switch that replaced it, and restores nothing | waited out, its tail lands in the next load: the engine flushed under a playing transport, which stopped answering; the newer document carries the master |
| a load belongs to the engine it waits on, and the first document through a different link takes it over | the account's engine changes while the gate waits; a dry run asking a closed link waits for the deadline |
| the copy to the engine's machine runs before the document is sent, off the engine's channel, with a progress bar | a download on the engine's channel holds every frame behind it, a `transport play` or a fader included, for as long as the file takes |
| what is copied is every absolute path the document names plus the records the open projects' playlists name and their `stems/`; the ledger is the bound machine's and counts size and mtime | a record from the playlist is already on the machine when it goes on a deck; binding another machine forgets the ledger |
| a failed copy is fatal: nothing is sent, later documents are dropped, the reconcile leaves the session alone, and the one button of the dialog retries | a project missing a file plays with that clip silent, worse than one that does not start; nothing retries behind the person's back |
| a file the engine holds open is not a failure, and a machine that does not know `sync` downloads on the engine's channel as it always did | the new version lands beside the old one on Windows; the server may ship before every machine has updated |
| the note of what a session serves is per session, never global, and every engine call names its workspace | a global note names whoever opened last and leaves another person's reconnect stranded; `null` is the bare session, an empty room where a preview or an arm says nothing |
| the reconcile asks the engine (`list-sessions`, per session and throttled) rather than trusting the note | the engine restarts under a long-lived service and a poll that trusts the note re-sends nothing for ever |
| an engine not holding the session is sent the project again; one that does not answer is a zombie link, dropped and re-dialled; three re-sends without the engine taking the project is a deaf socket, dropped and re-dialled | re-sending cannot land on a zombie, and a link can answer `list-sessions` while every project lands nowhere; three, because one re-send that loses a race with a restart is ordinary |
| only `loaded` is compared, never which project | the wire carries a display name and this side a path |
| the reconcile neither asks, re-sends nor re-dials while a copy holds the document or the load is `applying` or `warming` | the engine answers behind the document it is building, so a slow song reads as a zombie and the re-dial sends the document onto a busy link |
| a project that is gone from disk changes nothing when a late send for it arrives, and one that exists but does not compile is still noted | a late render or reconcile must not note a dead folder over the one served; the reconcile needs to know which project to try again |
| one socket per session, never a driver | each session gets its own outbox and reconnect; a driver counts as someone watching and no project would ever look abandoned |
| an idle link is reaped but its session is dropped only when the work is over; a session in `served` is never reaped | an abandoned link is a permanent reconnect loop and keeps its session alive on the engine; a person may come back to a loaded project |
| the reconnect ceiling is 4 s and jittered, the browser's 30 s is not borrowed | the service dials `127.0.0.1`; the engine restarts on every C++ save in development and a 30 s backoff reads as the engine ignoring an edit |
| on open the link replays the machine's preferences: the audio device first, then how its inputs are grouped | opening a device rebuilds the wave devices, so a pairing sent first is thrown away |
| a wait for a broadcast reply picks out its own: an arm by track, a plugin's parameters by id, and releases its listener whichever way it ends | the replies reach every client of the engine; another surface's answer would write one plugin's knobs into another plugin's file |
| a plugin's parameters answer three ways: no answer, found false (the machine cannot find or load it), found with none | a plugin that will not load needs a licence, and reported as a plugin with nothing to offer it sends nobody to look at the licence; a frame without `found` reads as loaded |
| the native window and the snapshot of a plugin's base are awaited, with different timeouts (30 s, short) | there is no reply that says buffered; a snapshot decides whether the caller may say a base exists |
| the machine's own questions (`/plugin-params`, `/plugin-native-ui`) ride the bare session and answer 503, never an empty list, when the engine cannot be reached | what a plugin has is a fact of the machine, asked while no project is open; an empty list is a real answer a panel would be written from |
| the plugin parameters take format and id together | the id is the binary's path and a display name can belong to two formats |
| the native window opens an orphan instance, nothing it is told is saved, and it draws where the engine is | it is somewhere to look at a plugin, not to set one; values belong in the module's text |
| a browser's document is handed to the link, which holds it per session until the project changes, and a reload or a switch overrides it | two writers to one engine make the last one win; the turntables surface would blink empty until its next gesture |
| the pattern watch re-sends the document only for a file a pattern lane plays, over a settle of one recompile, and drops what the project's own skip rule hides | a lane's pattern travels in the document; narrowing the predicate fails silently, so it is tested, and there is no second list of what is not part of a project |
| `/engine/served` answers in addresses, one project for the whole account, and null for a workspace with no address | the session is per account, and a row nobody could follow is worse than none; a menu that names a project the engine just dropped is corrected within one tick |

## The referee

| check | what it proves |
|---|---|
| frame-trace tests: each scenario run against the native link and a fake engine of its own that records every frame and answers from a script, compared with traces recorded once from the TypeScript | the link sends what the TypeScript sends, in its order, and the poll answers what the TypeScript's poll answered |
| a live harness against the real engine binary on a free port, silent, in a scratch home | the protocol, the dry run, a re-dial after the engine restarts and the scan gate on a real scan |

## On disk and on the wire

The engine's wire is unchanged (`apps/k7-daw-engine/k7-runtime/docs/protocol.md`, `apps/k7-daw-frontend/packages/engine-protocol`): the same frames,
the same session ids, the same stamps. The catalogue copy a finished scan writes is the TypeScript's own shape and
bytes, left alone when it says the same, since its mtime is in every project's revision token.

## The vectors

`tests/vectors/engine.json` is frozen: harvested once from the TypeScript, one case a line, replayed by `engine_vectors.rs` and never regenerated.

| section | what it holds |
|---|---|
| `session_of`, `session_for`, `served_project` | 392 roots (the files root, the bench, another workspace, outside it, `C:/` and UNC spellings), 7 owners, 4 homes; `servedProject` over 1,260 cases of owner, root, project and asker |
| `backoff` | `jitteredBackoffMs` at five draws of `Math.random`, three caps, attempts 0 to 12 |
| `load`, `scan`, `copy`, `reconcile` | 19, 12, 9 and 7 scenarios as traces: a script of events and answers in at stated times, the engine calls, the phases, the poll's view and the console's lines out, under a turned clock (the load's traces run the real `sendDoc` and `warmDeps` on a link that records every frame, so record, a send mid dry run and a new address are traces too); the 120 s apply bound, the 60 s stall and the 10 minute scan bound among them |
| `corpus` | over the 172 engine documents of the golden corpus, with their folders made absolute by `roots`: `collect`'s paths, the take hole for the busiest lane at three punches, the groups' mix |

## The rows as built

| row | where it is held | what a failing bug looks like |
|---|---|---|
| 2201 | `k7-file-service/src/engine/`: `load`, `scan`, `sync`, `reconcile`, `wire` and `docs`, each taking events and a clock and answering effects, none touching a socket, a timer or a file | the traces replay the TypeScript's; a record not put back after the dry run, a resend bound off by one, a scan bound doubled, a failed copy retried unasked and a frame for another session all fail one replay each |
| 2213, 2214, 2217 | `wire.rs`: the session filter, the outbox bounds, `state` kept natively | `engine_wire` and the hostile frames |
| 2220 to 2225 | the five machines, replayed from `engine.json` | `engine_load`, `engine_scan`, `engine_sync`, `engine_reconcile` |
| 2224 | `sync.rs` states only paths inside the roots; `collect` walks a document once with a set | a hundred thousand paths and a 64 KiB one are walked inside the backstop; a quadratic walk fails that test |
| 2230 | `apps/k7-daw-wasm/song/src/send_refs.rs` and `clips/audition.rs` | `send_refs`: the small cases and the audition replay in the unit run, the 177 projects in the ignored gate |
| 2231, 2232 | `docs.rs`: the group fold, the take hole and the audition applied to a document the typed reader takes, a document it refuses sent with the fold alone | `engine_vectors` over the golden corpus |
| 2210, 2211, 2250 | `k7-front/.../native/engine/mod.rs`: one `EngineLink` per account (sessions, the binding, the copies), its state under std mutexes taken for a decision and never across an await, a compile, a dial or a stat; `bind` keeps the hosted rules | `tests_link`' binding test (the same URL with a link open keeps the link; another drops it, no `drop-session`; none clears the jobs path and the identity) |
| 2212 | `jobs.rs`: the `sync` job over the bound path through `request_job`, which now takes a channel trait the tunnel's `Link` and a socket both implement | `tests_hostile`: a job never answered ends on its bound, a refused port and a refusal are failures with the machine's words |
| 2214 | `socket.rs`: one socket per session, jittered doubling capped at 4 s and reset by a socket that opened, the outbox replayed oldest first before the open is told, frames up to 100 MiB read as the `ws` library reads them | `tests_link`: a 5 s flap is four to six dials; frames sent while the engine is away arrive in order; a 16 MiB frame does not stop the session |
| 2216, 2252 | `queue.rs`: every frame leaves through the session's socket channel, pushed while the session's lock is held, so the order is the lock's and a push is a channel send, never a write; documents wait in `copy.rs`' gate in the order asked | `tests_link`' dry run on the wire: the document, the silenced copy at -100, the restore, one play |
| 2217 | `queue.rs`: `state` is kept (`playing`, `recordEnabled`), `plugin-scan` and `plugins` fed to the machines, `recorded` filed from a task of its own, any other type only while something listens, and `state` never | `tests_link`' state test; `tests_hostile`' flood of 10,000 and a filing that hangs |
| 2228 | `patterns.rs` on `Watch::listen`: the watcher of 1c hears every path below the project before its filters, and a settled burst recompiles when `pattern_files` names the file | `tests_flow`' pattern test: a pattern nothing inlines recompiles nothing |
| 2253 | a link is one number per (session, binding): kept across the reconnects of its socket, new after a re-dial or a change of binding, which is when the machines hear it closed | `tests_link`' blip test: a drop and reconnect mid-apply keeps the link and the one dry run |
| 2261 | `tests_live.rs`, ignored, against the real engine behind a recording tap: the dry run's three documents (the master at -100 between), a switch, the scan gate on a real scan with its catalogue, and an engine killed and started again on its port | run with `K7_LIVE_ENGINE_URL` (and `K7_LIVE_ENGINE_BIN` for the kill) |
| 2262 | `engine/stats.rs` counts loads, dry runs, re-sends, re-dials, copies, polls and panics; each account's link logs `[files] native engine <email>: loads= dry= resends= redials= copies= polls= panics=` every 30 s once it moved and the counters changed, at once when a panic is counted and when the link ends; `bedCounters.mjs` sums each account's last line and holds `loads` 20, `dry` 10 and `polls` to a quarter of a poll per second of the web suite (a conformance-only bed holds `polls` to 1 and `dry` to 5: its polls are the browser's workspace poll, which that suite does not run, and its dry runs no longer include the second prefetch's), and `panics` to 0 as the maximum over every line, at the end of every bed script | a bed that loaded nothing must not read green; the browser polls every two seconds while a project is served, so polls follow the run's wall clock and a floor on them is a rate (`e2e/docs/e2e.md`); a floor is raised from a printed run, never lowered |
| 2263 | `engineLink.spec.ts`, run against the suite's stand-in engine: the poll's keys and `engineLoad` through an open, `/engine/served` before and after an unload, a browser's document then a reload, a failed copy and its retry, and a LOAD held by the plugin scan (an unload, then an open: a switch on a bound engine goes at once and holds nothing). `tests_link`' applying test holds what the spec cannot see without a race: the poll says `applying` while the engine has not loaded the project, and the dry counter counts it | the bed's spec; a poll that says `null` while applying fails `tests_link` |

The link asks the rest of the front through `Host` (compile, revision, the external-plugin question, the files a project names, the pattern files, the preferences, the catalogue's write); each call may do I/O and is made from the blocking pool. What it tells goes through `Tells` (a recorded take, a forwarded frame, the module watch, a console line): the files take and the says lines, enqueued inline so the order of the engine's words is kept and a slow filing holds neither a later event nor the reader (D6). A fake engine and a fake jobs agent (`fake.rs`) speak the real protocol over loopback sockets.

The lock order is `sessions`, then a session's `st`, then the leaves `binding` and `copies`; nothing is taken under a leaf, and `bind` reads the open sockets before it takes the binding. A frame asked while a document is still being decided (stat, compile, the external question) waits for it and leaves right after; a document the copy or scan gate has parked holds nothing (`hold.rs`, D2).

Two readings of the engine are choices: the engine is not asked by a socket the poll finds still dialling, and a document already sent twice at a first open (the open re-sends what the session serves) is the TypeScript's behaviour kept, not a duplicate to remove.


A pattern file that is not yet written is still named by `pattern_files`, as the TypeScript names it, so its creation is a recompile.
A model reads a socket as opened on a later turn than its dial, so the first poll after a dial asks nothing.

| row | where it is held | what a failing bug looks like |
|---|---|---|
| 2242 | a take the engine reports is noted with the project the session serves, queued at once on its session's own channel, whose one worker files them in the order they ended (`edits::take`, landed whole under the account's write lock, no journal step) and, once the clip is in, recompiles and sends the project; the filing's answer is the session's `lastTake`; a take with no served project is a `[rec]` line | `tests_host`' forward-and-take test; `tests_hostile`' takes in order |
| D1 to D4 | `socket.rs`: frames past 16 MiB close the socket; a write past 10 s drops it into the bounded outbox; the backoff starts over only for a socket that stayed open 4 s or carried a frame | `tests_hostile`: a 17 MiB frame, a stalled engine, an engine that opens and closes at once |

## The routes and the host

| row | where it is held | what a failing bug looks like |
|---|---|---|
| 2210, 2211 | `services.rs`: `Held` holds the account's `Engine` (link, bridge, host); `Services::bind` tells the link; the stop closes it | `tests_real`: an open reaches the engine through the bridge |
| 2226, 2227 | `routes.rs`: the poll (1g's info, then `lastTake`, `engineSync`, `engineLoad`, `engineScan`, `processes`), `/engine/project`, `/engine/unload`, `/engine/sync/retry`, `/engine/served`; the poll's prewarm is one job of the session's queue at a time, never awaited, and begins the load under the lock so the poll that starts it answers `copying`; the machines' Resend, Restore, Recompile and Opened are jobs of the same queue, and a compile holds `deciding` from before it starts | `engineLink.spec.ts` at the bed |
| 2216, 2230 | `real.rs`: the host compiles with the account's `SongSessions` on a thread with the compiler's stack, reading the revision token from `Revisions` and the thawed list from the account's jobs; a compile that fails is a failure the link reports | `tests_real` |
| 2215 | `rig::engine_prefs` reads the device and the input pairs of `preferences.k7x` at each socket open, on the blocking pool | `tests_review`: a redial replays the preferences as they stand |
| 2247, 2248 | the open starts the link itself (`engine/starts.rs`: the marker's project, a queued start); the open waits (10 ms steps, 2 s at most) until the session's load has begun or it serves the opened project, and answers the link's `engineLoad`; a save's push is the front's own call of the same start after the host dropped its document | `tests_review`: the open of a second project answers a phase; the bed's `engineLink.spec.ts` |
| 2273 | A verb asked on a session with no socket yet dials it: the readiness check opens the socket and answers whether it is open, so the first ask may say "engine offline" while it opens and the next is answered | `tests_review`: a verb with no project started dials the engine |
| 2272 | The `unloadUnder` ask is a job of every session that serves or has work pending, answered once all ran; a start queued before it is decided first | `tests_order`' unload under a folder; a prewarm queued after a start of another project finds that project served |
| 2275 | The take's arm is read when the engine reports the take and queued with the take, so a disarm told before the filing runs does not move the take; the arm stands for that filing alone | `tests_review`: a take is filed under the arm held when it was reported |
