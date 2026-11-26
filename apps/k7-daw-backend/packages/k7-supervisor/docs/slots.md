# Slots in the Rust supervisor

What a slot IS, the files it is stored as and the commands that set it are `apps/k7-daw-engine/k7-supervisor/docs/slots.md`
(it moves beside this page at the switch); the layout under `<state>/slots/` is byte for byte the same, so a machine
keeps its slots across the change. This page is what the Rust part chose.

| choice | why |
|---|---|
| one part, `parts/slots`, holds the rows, the enabled mark and the follow; the `slot.*` commands and a server's tunnel jobs are the same verbs, as `Op`s | the C++ loop owned the state and a job reached it by posting to the loop; a part is one task that owns it and a verb is a message, so a command and a job can never disagree |
| a job reaches the part through `parts::slots::handle(paths)`, a map keyed by the state folder | the jobs run under the tunnel part's `Ctx`, and a part sees no other part; keyed by the folder rather than one global, so several cores in one test process each have their own |
| the part builds its own `EngineClient` and polls the sessions itself, every 2 s, and only while a slot is sent and not yet seen gone | the queries part's poll is its own and shares nothing with the slot's; asking only while something is followed leaves an idle machine silent on the engine's socket |
| the slot is sent once the engine's port accepts (30 s at most), not the moment the core says it is up | a spawned engine is up in the core's eyes before it listens; the wait is dropped when the engine is replaced or stopped meanwhile |
| what the machine has installed is asked of the engine at the save (`list-plugins`), and of the disk for instruments | a part does not see the snapshot; with the engine down every third-party plugin is missing, and the save is refused naming it, as it was with an empty catalogue |
| a server's save may copy only what `guard::mapped` accepts, the tunnel's copies of the server's folders | one rule of "a server's path" for every job, resolved through links; it is the C++'s rule or stricter |
| a save runs the copy on `spawn_blocking` and answers when the part has listed it, and sent it when it is the enabled slot | an SFZ's folder can be large and the part must keep answering; a refused save leaves the slot it would have replaced |
| `slot.disable` and `slot.remove` answer after `drop-session` has been SENT, not acknowledged | the engine says nothing back to it; the C++ answered at the same point |

## How it is tested

`tests/all/slots.rs` holds the store and the files (`Slots_test`'s cases and the refusal of a file outside the server's
copies); `tests/all/slots_part.rs` runs the part in the whole core against `k7-ws-engine`: the slot sent at the first start
and again after a stop and a start, a project taking over (the session gone after it was seen loaded) disabling it, every
`slot.*` command with what the engine received; `tests/jobs_part.rs` runs the six jobs over a hub.
