# Slots: the frozen chains a machine keeps compiled

A **slot** is a frozen chain compiled once into the document the engine plays, kept by this
machine's supervisor. At most one slot is **enabled**: the engine plays it in its `slot` session with
no page, no file service and no network, and plays it again after every start. The design and the
lots still to build are `docs/engine-slots-plan.md`.

| choice | why |
|---|---|
| the supervisor keeps the slots, in `<state>/slots/<id>/slot.json`, the enabled one named by `<state>/slots/enabled` (`../src/Slots.cpp`) | it owns the machine's lasting settings and starts the engine; the engine keeps nothing between runs |
| a slot file is `{id, name, chain, savedAt, protocol, doc}`, `doc` the compiled `ProjectDoc` of a live rig around the chain | the engine reads no other form; the chain and the protocol say what to recompile it from, and with what |
| written beside, then renamed over | a crash mid-write leaves the last whole slot |
| **a slot carries the files it plays**: every file its document names — a NAM or AIDA-X model, an IR, a Surge patch, a base preset, a rack's samples — is copied into `<id>/files/` and its path rewritten there (`../src/SlotFiles.cpp`) | a slot made on one machine plays on another that has none of its paths, and deleting the chain or its library takes nothing from it |
| a plain `.sfz` is copied with its whole folder | its samples sit beside it, unnamed in the document |
| an `@instruments/` SFZ stays the machine's own, and must be installed | it is a downloaded instrument (`instruments.md`), as big as a library; a slot names it and the machine has it or not |
| a third-party plugin (VST3, AU, CLAP, LV2, LADSPA, AAX) must be in the machine's plugin scan | the document names it by format and name only, as the engine looks it up (`engine/k7-runtime/engine/EditBuilder.cpp`): it cannot travel, so the save is refused, naming it, rather than the slot playing silence |
| pattern lanes' sample banks refuse the slot | they are folders named by a path the slot cannot follow, and a live rig plays none |
| `presetSave` emptied, `recordDir` moved to `<id>/takes` | a slot never writes back to what it was saved from, and the compiling machine's record folder is not this one |
| the copy runs off the supervisor's loop, what is installed read on it first | an SFZ's folder can be large, and the loop must keep answering |
| a refused save leaves the slot it would have replaced as it was | the whole slot is built in `<id>.new/` and only then put in place |
| an id is lowercase letters, digits and `-`, 64 at most | it is a folder name on every platform, and a path cannot be smuggled in through it |
| the enabled slot is sent at every engine start, in a `project` message on session `slot` (`EngineQueries::applyProject`) | a machine boots into its chain, and a relaunched engine finds it again |
| it counts as playing once a sessions poll shows it loaded; seen gone after that, the supervisor forgets it is enabled | the engine drops the slot when a project opens on it (`engine/k7-runtime/docs/sessions.md`, "The slot"); waiting for the first "loaded" keeps a slot still being applied from reading as dropped |
| saved again while enabled, the new version is sent at once | "update the slot" means hearing the update |
| the app hands a slot over by a staged FILE (`slot.save <path>`), the file consumed once stored | a document is far bigger than the one-line commands the app sends (`hostProtocol.md`) |
| the state frame lists the slots without their documents, and `enabledSlot` | the app draws names and dates; the document is the engine's to read |
| a server sets a machine's slots as tunnel jobs — `slots`, `slot-save`, `slot-enable`, `slot-disable`, `slot-remove` — each answered with the store as it then stands (`../src/MachineJobs.cpp`) | the owner's page may be on that server, with no backend on this machine to stage a file; the tunnel is already how a server asks this machine for work |
| a slot's footswitch is a MIDI CC stored in its `slot.json` (`slot.footswitch <id>\x1f<cc>`, the `slot-footswitch` job), kept when the chain is saved again | the pedal belongs to the machine's pedal board, not to the chain: updating the sound must not unbind the switch |
| sent, it rides in the document as `footswitch: {cc, plugins}`, `plugins` the `chainPlugins` the compiler listed (`apps/k7/backend-rs/crates/k7-file-service/src/bench/slot.rs`) | the engine bypasses the chain's own plugins and never the rig's fader or recorder; a slot saved before the compiler listed them has nothing to bypass until saved again |
| binding the enabled slot sends it again at once | the engine binds the pedal when a document arrives |
| a job names its slot by `slotId` | the job's own `id` is the channel's, matching a reply to its request |
| a server's save copies only files under `<state>/remote/`, what the tunnel brought down before the job ran | a server must not get a file of this machine into a slot by naming its path |
| the server lists an SFZ's neighbours as `assets` in the slot it sends | the tunnel fetches only what a job names, and the samples are named only inside the `.sfz` |

The commands are `slot.save <staged file>`, `slot.enable <id>`, `slot.disable` and
`slot.remove <id>`; removing the enabled one disables it first. Which page sends them, and how a chain
becomes a slot, is `apps/k7/frontend/docs/slots.md`; the server's routes are
`apps/k7/backend-rs/docs/engine/slots.md`.
