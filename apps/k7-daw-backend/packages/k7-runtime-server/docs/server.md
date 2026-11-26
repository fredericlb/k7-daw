# The socket core

`src/server/` is the engine's WebSocket server: what stands between a client's socket and the
engine's C ABI. The wire it speaks is every installed client's, byte for byte (`protocol.md`);
who may drive is `pairing` (`pairing.md`); the engine behind it is `abi.md`.

## One task pair per socket

- **A socket owns its queue and its writer.** One writer shared by every socket would let a stuck peer delay
  every other socket's JSON by the send timeout; per socket, a peer that stops reading costs only itself,
  and is closed after 5 s.
- **A reply is kept, a state frame is droppable.** A droppable frame is refused while the socket has more than
  64 KiB unsent; a reply is refused only past 32 MiB, which means the writer is stuck. The caps are per socket.
- **A ping every 10 s, and a peer that answers none for a whole interval is closed.** tungstenite does not
  ping by itself; any frame received counts as an answer.
- **The reader runs the handler inline.** A handler must not block: work for the message thread is posted,
  anything that waits is spawned.

## Order of a frame

The size screen, the parse (64 levels of nesting, counted on the text before the parse, so a deep message
is never walked), the gate, the session's length and ceiling, the deferred `[client] connected` line and
the subscription, then the handler. The probe and override sets decide who creates a session, who earns
the line and who binds the socket's subscription (`wire::catalogue`).

- **A failure is answered, never thrown.** A handler's `Malformed` (a field of the wrong type, a message
  that is not an object) and a panic are both answered `malformed message: …` and logged; the socket stays.
  The text copies nlohmann's `type_error` wording, since a client may show it.
- **Names, not pointers.** A socket is a counter that is never reused, so a late frame cannot reach the next
  connection that took its place.

## Startup and stop

- **The port is bound before the host's loop runs**, and so before the audio device opens, which is queued
  as the loop's first job: a refused connection reads as an absent engine. `whoami` is answered on the
  socket's own task from any-thread reads, so it answers with the loop not yet running.
- **`start` and `Running::run` are split** so a test holds the loop back; `serve` is both.
- **SIGINT and SIGTERM post a quit from a task**, never from the handler; on stop every socket is told
  `1001 going away` and given a moment to hear it.

## The executable's exit

An executable whose detached work (a plugin scan, a probe) still runs when the server lets go of it leaves with
`_exit`, without unwinding: that work holds what the process's static destructors free. Inside an app, where
exiting would take the app with it, the server only returns.
