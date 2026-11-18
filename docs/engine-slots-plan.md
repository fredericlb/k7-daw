# Engine slots: a frozen chain the engine machine keeps

The decisions of the engine-slots work, in one place, and the lots it is built in. A **slot** is a
frozen chain compiled once and kept by the engine machine, which plays it on its own when it is the
enabled one: no page, no file service, no network.

## The goal

- A chain played on an engine needs nothing else to stay up: no bench page, no file service serving
  it, no tab left open, and it survives an engine restart.
- A headless engine — a Pi in a pedal board, a machine with no screen — boots into a chain and plays
  it, toggled from a footswitch.
- Designing a chain stays what it is: the bench, then `chain save` (`apps/k7/frontend/docs/frozenChains.md`).
  Saving it into a slot is one more step, from the bench or the chains page.

## What forces the shape

A chain played on k7's bench lives only as long as the page that opened it, its file service and its
network: close the tab, lose the network, or restart the engine, and it is gone. The engine itself
keeps nothing between runs, and a
Pi plays only what a remote file service sends it (`docs/pi-engine.md`). So the chain must be kept
where it plays, already compiled — the one form the engine reads (`engine/k7-runtime/AGENTS.md`).

## Decisions

| choice | why |
|---|---|
| the SUPERVISOR keeps the slots, under its state folder, and loads the enabled one into the engine at every engine start | it already owns the machine's persistent settings and starts and relaunches the engine; the engine stays stateless, so a crash rebuilds nothing it held |
| a slot is the compiled `ProjectDoc` of a live rig around the chain, with the chain's ref, the compiler's version and the date it was made | the engine reads nothing else; the ref and the version let k7 recompile it when the engine no longer reads the form it was saved in |
| a slot carries the files it plays — NAM and AIDA-X models, IRs, SFZ samples — in its own folder, its paths rewritten there | a slot made on a desktop must play on a Pi that has none of that desktop's paths |
| a slot needing a plugin the machine lacks (a third-party VST3) is refused when saved there, saying which | silence is worse than a refusal |
| **at most ONE slot is enabled**, or none | an engine is one instrument chain at a time: a pedal, an instrument's input; two would monitor the same inputs twice |
| **opening or editing a project on the engine disables the enabled slot**, the engine telling the supervisor so | the engine is then the project's: a slot left playing under it would double the inputs and confuse what is heard |
| the enabled slot is remembered, so a machine boots into it | a pedal is unplugged, not shut down |
| a slot plays in a session of its own (`slot`), never let go for having no page, never reaped | it exists precisely to play with nobody attached |
| saving into a slot is a SNAPSHOT: a later edit of the chain does not reach it until saved again; k7 offers "update the slot" when the chain is newer | a pedal's sound must not change because a file synced |
| only the engine's owner writes or enables its slots, by the ownership the registry already applies (`apps/k7/backend-rs/docs/engine/ownership.md`) | a shared engine is not anyone's pedal board |
| a slot is enabled or disabled from k7's engine page, through the registry for a remote engine, and from a MIDI footswitch or CC the slot names | a pedal has no screen |

## Lots

1. **Slots on one machine** (built). The supervisor's store and its commands (save, list, enable, disable,
   remove); the engine's `slot` session and its rule that a project's `apply` disables it; k7's
   "save to a slot" on the bench and the chains page, and the slots on the engine page. Proof: a
   slot enabled, the engine killed, the chain heard again after the relaunch with no page open.
2. **Slots that travel** (built). The files a slot plays copied into it, its paths rewritten, the plugin
   check on save; saving a slot onto a remote engine through the server holding its tunnel.
3. **Headless** (built). The footswitch binding, and the Pi image booting into its enabled slot.

## Open

- A slot made by an older compiler: recompiled by k7 when a file service is reachable; offline, the
  engine refuses it and says the slot must be saved again. Whether the engine can play an older form
  as is depends on how `ProjectDoc` changes, and is decided when it first changes.
