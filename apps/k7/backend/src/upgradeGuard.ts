// upgradeGuard.ts — the WebSocket upgrades of this server as one rule, ahead of every listener: each upgrade
// socket answers its own errors, and one on a path nothing here serves is refused with a 404 at once.
// The one thing a reader would get wrong: each listener ignores what is not its own (../docs/upgrades.md).

import type { IncomingMessage, Server } from "node:http";
import type { Duplex } from "node:stream";
/** Every path a listener of this server takes an upgrade on. */
export const SERVED_UPGRADES: readonly RegExp[] = [];

const pathOf = (req: IncomingMessage): string => {
  try {
    return new URL(req.url ?? "", "http://localhost").pathname;
  } catch {
    return "";
  }
};

/** Put the rule in front of the server's upgrade listeners, whenever those are added. */
export function guardUpgrades(
  server: Server,
  served: readonly RegExp[] = SERVED_UPGRADES,
  log: (line: string) => void = console.warn,
): void {
  // A client that resets is the commonest thing on a network; on a socket an upgrade took from Node's
  // own handling, nothing else listens, and the error would end the process. Per connection, so the
  // order the listeners below are added in cannot leave one out.
  server.on("connection", (socket) =>
    socket.on("error", (e) => log(`socket: ${e.message}`)),
  );
  server.prependListener("upgrade", (req: IncomingMessage, socket: Duplex) => {
    if (served.some((r) => r.test(pathOf(req)))) return;
    socket.end("HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n");
    socket.destroy();
  });
}
