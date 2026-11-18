# The WebSocket upgrades: one rule in front of the listeners

This backend takes no WebSocket upgrade of its own: the engine relay, the engines' tunnels, the remote cameras and
a share's player are the Rust front's, and `/api/sync` answers 404. The guard stays so that any listener added
here is covered. An upgrade skips Express, so none of Express's fallbacks or error handling reaches it.

A listener ignores a path that is not its own, as it must, since every listener hears every
upgrade. Node closes an unclaimed upgrade only when no listener exists at all. So with any, an upgrade
on any other path is left open: never answered, never closed. That has two costs:

- **A client waits for good.** A socket the server routes nowhere looks, from the page, like a
  server that is slow to answer, so nothing on the page ever reports a failure.
- **A reset ends the process.** Once an upgrade is detached from Node's HTTP handling, nothing
  listens for the socket's errors, and a client that goes away with a reset (a closed tab, a phone
  changing network) throws an uncaught `ECONNRESET`. The backend exits on an uncaught exception, so
  a server is restarted by its container, but the tray and the phone are left without their backend.

`src/upgradeGuard.ts` answers both, ahead of every listener:

- Every connection gets an error listener when it is accepted. A connection, not an upgrade: an error listener
  put on the upgrade would depend on the order the parts started in.
- An upgrade on a path no listener serves gets a `404` and is closed at once.

The paths are listed in one place (`SERVED_UPGRADES`, empty now), from each part's own constant. A part
that takes a new upgrade path adds it there, or its sockets are refused; `upgradeGuard.test.ts`
names every one.
