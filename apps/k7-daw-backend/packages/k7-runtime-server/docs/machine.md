# The machine: sessions, interface, broadcast, render

`src/server/machine.rs`, `broadcast.rs`, `audio.rs` and `render.rs` answer the verbs about the machine rather than
one project, over the `machine` and `render` areas of the ABI (`abi.md`). The wire is every installed client's, byte
for byte (`protocol.md`).

## The broadcast has one consumer thread

- **The sender is an OS thread, not a task.** The master tap's ring has one reader; a task may move between
  threads at every await, so the single consumer is a thread of its own, ticking every 5 ms and draining up to
  32 blocks of 480 frames a pass.
- **An arrival is queued for the sender.** A first listener starts from live, and moving a read cursor belongs to
  the sender alone, so the verb only records the session and the sender skips the backlog on its next pass.
- **A frame is posted, never sent.** It is encoded once per codec asked for and queued binary on each listener's
  outbox, skipped for a socket with more than 512 KiB unsent. A hole the tap overwrote is a jump in the session's
  sequence number, rounded up to whole blocks, so a listener re-cushions.
- **A tap with no sample rate is withheld**, said once per session: the reader rejects a rate of zero.

## Every post about a listener is made under the bindings lock

A departing listener's `false` must not land after an arriving one's `true`. The `broadcast` verb and a socket's
departure (`broadcast::leave`, called by `Ctx::forget`) both post to the engine with the lock held, which is also
how a closed recorder tab, which says no goodbye, releases the interface it silenced and the tap it left on.

## What shares a query

`list-sessions`, `list-audio-devices` and `io-levels` join a query already waiting on the message thread, whose
answer is a broadcast. A device list unchanged since the last one sent answers its asker alone; one that moved, or
that a second asker joined, goes to everyone. `audio-cost` and `instrument-status` read atomics and files and so
answer on the socket's own task.

## Render

- **Only the machine's own clients ask.** A render writes files and takes a core; the check is the gate's locals.
- **Refused whole.** The doc is parsed as a project is, after its relative clip sources are joined to `base`; one
  path outside the roots refuses it, since a render missing a clip is a wrong file and not a warning.
- **One at a time, on a thread of its own.** The bounce blocks and needs the message loop running, so it never
  runs on a socket's task; a second ask is answered `busy`. At stop the renders are cancelled and waited for briefly.
- **The supervisor's request is unchanged**: `{type, id, doc, outDir, stems}`, answered `{type: "rendered", id, ok,
  manifest | error}` to the asking socket alone.
