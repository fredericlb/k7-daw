// The k7 engine wire protocol — the payload SHAPES shared by the two WebSocket
// clients of the C++ engine (the Node one and the browser one), so they can't silently
// drift. Types only: the message-type strings stay in each client, since every surface
// implements its own subset and the engine itself is the authority on the full contract.
// Being type-only, this module and ./project are erased at runtime and cost nothing to
// share. ./build is the exception — constructors, so it emits code. The frames' shapes are
// generated from the engine's Rust structs (./generated); only TransportAction is written here.

export type * from "./generated";

// The bare transport verbs both clients send. A positioned "seek" carries a field, so it goes as its own message.
// `panic` is a transport verb because it is what a STOP that did not work asks for next — see
// apps/k7-daw-engine/k7-runtime/docs/panic.md.
export type TransportAction = "play" | "stop" | "toggle" | "rewind" | "panic";
