// socket.ts — `openSocket`, the one place the engine's WebSocket is made, so a test can stand in for it.

/** A WebSocket for an engine's address. */
export function openSocket(url: string): WebSocket {
  return new WebSocket(url);
}
