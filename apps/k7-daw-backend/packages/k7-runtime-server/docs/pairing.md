# Pairing in the Rust server

Who may drive the engine, and the flow, are `apps/k7-daw-engine/k7-runtime/docs/pairing.md`; this
page holds the choices of the module that implements it (`src/pairing/`). The plan it belongs to is
`docs/rust-engine-server-plan.md`.

## The gate owns no socket

`Gate` takes a socket's id (`SocketId`, a `u64` the socket core numbers), the peer address and the
parsed message, and returns what to do (`Out`): reply to the sender, send to one socket, send to
every loopback socket, close a socket. The socket core carries it out.

- **Pure, so it tests with no network.** Every behaviour of the C++ gate is a test that calls
  `screen_message` and reads the `Out`s.
- **Ids, never pointers.** The C++ store keyed on `ix::WebSocket*`; a number cannot dangle, and a
  closed socket's id is dropped by `forget_socket`.
- **`Out` is ordered.** An approval sends the key to the device before it refreshes the musician's
  lists; the core sends in the order given.
- **Trust is the gate's, the verbs are the table's.** `screen_message` answers `None` for a verb the
  gate does not own, and the handler table runs it.

## The store is the C++ store's file

`paired-devices.json` is read and written by both engines while the C++ one exists, and by the
Rust one alone afterwards, so the file is `dump(2)` of nlohmann byte for byte (keys sorted, two
spaces, no trailing newline), written aside and renamed, readable by its owner only.

- **Held by fixtures written by the C++ store** (`fixtures/paired-devices.*.json`), read here and
  rewritten identical.
- **A store of another shape pairs nobody**, as in C++: any field of the wrong type empties the list.
- **The clock is a function pointer**, so a test moves time for the two-minute expiry and the hourly
  save of "last seen" without sleeping.
- **Keys and codes come from `getrandom`**, the OS generator, as `std::random_device` did.

## A name is clipped in bytes

A request's name is cut at 64 bytes as C++'s `substr` cut it, and a character cut in two prints as
U+FFFD, which is what the C++ printer's `replace` handler wrote.
