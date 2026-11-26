# Slots: the frozen chains a machine keeps compiled

A **slot** is a frozen chain compiled once into the document the engine plays, kept by this
machine's supervisor. At most one slot is **enabled**: the engine plays it in its `slot` session with
no page, no file service and no network, and plays it again after every start. The design is in this page.

| choice | why |
|---|---|
| the supervisor keeps the slots, in `<state>/slots/<id>/slot.json`, the enabled one named by `<state>/slots/enabled` (`src/slots/`) | it owns the machine's lasting settings and starts the engine; the engine keeps nothing between runs |
| a slot file is `{id, name, chain, savedAt, protocol, doc}`, `doc` the compiled `ProjectDoc` of a live rig around the chain | the engine reads no other form; the chain and the protocol say what to recompile it from, and with what |
| written beside, then renamed over | a crash mid-write leaves the last whole slot |
| **a slot carries the files it plays**: every file its document names — a NAM or AIDA-X model, an IR, a Surge patch, a base preset, a rack's samples — is copied into `<id>/files/` and its path rewritten there (`src/slots/files.rs`) | a slot made on one machine plays on another that has none of its paths, and deleting the chain or its library takes nothing from it |
| a plain `.sfz` is copied with its whole folder | its samples sit beside it, unnamed in the document |
| an `@instruments/` SFZ stays the machine's own, and must be installed | it is a downloaded instrument (`instruments.md`), as big as a library; a slot names it and the machine has it or not |
| a third-party plugin (VST3, AU, CLAP, LV2, LADSPA, AAX) must be in the machine's plugin scan | the document names it by format and name only, as the engine looks it up (`apps/k7-daw-engine/k7-runtime/engine/EditBuilder.cpp`): it cannot travel, so the save is refused, naming it, rather than the slot playing silence |
| pattern lanes' sample banks refuse the slot | they are folders named by a path the slot cannot follow, and a live rig plays none |
| `presetSave` emptied, `recordDir` moved to `<id>/takes` | a slot never writes back to what it was saved from, and the compiling machine's record folder is not this one |
| a refused save leaves the slot it would have replaced as it was | the whole slot is built in `<id>.new/` and only then put in place |
| an id is lowercase letters, digits and `-`, 64 at most | it is a folder name on every platform, and a path cannot be smuggled in through it |
| the enabled slot is sent at every engine start, in a `project` message on session `slot` (`src/parts/slots/apply.rs`) | a machine boots into its chain, and a relaunched engine finds it again |
| it counts as playing once a sessions poll shows it loaded; seen gone after that, the supervisor forgets it is enabled | the engine drops the slot when a project opens on it (`apps/k7-daw-engine/k7-runtime/docs/sessions.md`, "The slot"); waiting for the first "loaded" keeps a slot still being applied from reading as dropped |
| saved again while enabled, the new version is sent at once | "update the slot" means hearing the update |
| the app hands a slot over by a staged FILE (`slot.save <path>`), the file consumed once stored | a document is far bigger than the one-line commands the app sends (`hostProtocol.md`) |
| the state frame lists the slots without their documents, and `enabledSlot` | the app draws names and dates; the document is the engine's to read |
| a server sets a machine's slots as tunnel jobs — `slots`, `slot-save`, `slot-enable`, `slot-disable`, `slot-remove` — each answered with the store as it then stands (`src/parts/tunnel/slot_jobs.rs`) | the owner's page may be on that server, with no backend on this machine to stage a file; the tunnel is already how a server asks this machine for work |
| a slot's footswitch is a MIDI CC stored in its `slot.json` (`slot.footswitch <id>\x1f<cc>`, the `slot-footswitch` job), kept when the chain is saved again | the pedal belongs to the machine's pedal board, not to the chain: updating the sound must not unbind the switch |
| sent, it rides in the document as `footswitch: {cc, plugins}`, `plugins` the `chainPlugins` the compiler listed (`apps/k7-daw-backend/packages/k7-file-service/src/bench/slot.rs`) | the engine bypasses the chain's own plugins and never the rig's fader or recorder; a slot saved before the compiler listed them has nothing to bypass until saved again |
| binding the enabled slot sends it again at once | the engine binds the pedal when a document arrives |
| a job names its slot by `slotId` | the job's own `id` is the channel's, matching a reply to its request |
| a server's save copies only files under `<state>/remote/`, what the tunnel brought down before the job ran | a server must not get a file of this machine into a slot by naming its path |
| the server lists an SFZ's neighbours as `assets` in the slot it sends | the tunnel fetches only what a job names, and the samples are named only inside the `.sfz` |

The commands are `slot.save <staged file>`, `slot.enable <id>`, `slot.disable` and
`slot.remove <id>`; removing the enabled one disables it first. Which page sends them, and how a chain
becomes a slot, is `apps/k7-daw-frontend/docs/slots.md`; the server's routes are
`apps/k7-daw-backend/docs/engine/slots.md`.

## How the supervisor's part keeps them

| choice | why |
|---|---|
| one part, `parts/slots`, holds the rows, the enabled mark and the follow; the `slot.*` commands and a server's tunnel jobs are the same verbs, as `Op`s | a part is one task that owns the state and a verb is a message, so a command and a job can never disagree |
| a job reaches the part through `parts::slots::handle(paths)`, a map keyed by the state folder | the jobs run under the tunnel part's `Ctx`, and a part sees no other part; keyed by the folder rather than one global, so several cores in one test process each have their own |
| the part builds its own `EngineClient` and polls the sessions itself, every 2 s, and only while a slot is sent and not yet seen gone | the queries part's poll is its own and shares nothing with the slot's; asking only while something is followed leaves an idle machine silent on the engine's socket |
| the slot is sent once the engine's port accepts (30 s at most), not the moment the core says it is up | a spawned engine is up in the core's eyes before it listens; the wait is dropped when the engine is replaced or stopped meanwhile |
| what the machine has installed is asked of the engine at the save (`list-plugins`), and of the disk for instruments | a part does not see the snapshot; with the engine down every third-party plugin is missing, and the save is refused naming it, as it was with an empty catalogue |
| a server's save may copy only what `guard::mapped` accepts, the tunnel's copies of the server's folders | one rule of "a server's path" for every job, resolved through links |
| a save runs the copy on `spawn_blocking` and answers when the part has listed it, and sent it when it is the enabled slot | an SFZ's folder can be large and the part must keep answering; a refused save leaves the slot it would have replaced |
| `slot.disable` and `slot.remove` answer after `drop-session` has been SENT, not acknowledged | the engine says nothing back to it |

## How it is tested

`tests/all/slots.rs` holds the store and the files (the store's cases and the refusal of a file outside the server's
copies); `tests/all/slots_part.rs` runs the part in the whole core against `k7-ws-engine`: the slot sent at the first start
and again after a stop and a start, a project taking over (the session gone after it was seen loaded) disabling it, every
`slot.*` command with what the engine received; `tests/jobs_part.rs` runs the six jobs over a hub.
