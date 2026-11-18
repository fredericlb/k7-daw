// front.ts — the Rust front (apps/k7/backend-rs) proxies to this backend, so every socket it opens here is
// loopback. A request carrying the front's secret has the peer the front saw recorded on it. One that
// names the front with a wrong secret, or reaches a backend never given one, fails closed: its peer is
// UNVERIFIED and no loopback check passes. A request with no front header is read from its socket, as
// the file services' own calls are (apps/k7/backend-rs/docs/front.md).

import { timingSafeEqual } from "node:crypto";
import type { IncomingMessage, Server } from "node:http";
import { FRONT_PEER } from "@studio/web-protocol/services";

const SECRET_HEADER = "x-k7-front";
const PEER_HEADER = "x-k7-peer";
/** The peer of a request that named the front without proving it: no address check accepts it. */
export const UNVERIFIED_PEER = "front-unverified";

/** The peer to record on `req`: the front's when it proves itself, UNVERIFIED when it fails to, undefined with no front header. */
export function vouchedPeer(
  req: IncomingMessage,
  secret: string | undefined,
): string | undefined {
  const given = req.headers[SECRET_HEADER];
  const peer = req.headers[PEER_HEADER];
  if (given === undefined && peer === undefined) return undefined;
  if (!secret || typeof given !== "string" || typeof peer !== "string" || !peer)
    return UNVERIFIED_PEER;
  const a = Buffer.from(given);
  const b = Buffer.from(secret);
  return a.length === b.length && timingSafeEqual(a, b)
    ? peer
    : UNVERIFIED_PEER;
}

/** Marks every request and upgrade before any other listener sees it; `secret` is `K7_FRONT_SECRET`, if set. */
export function trustFront(server: Server, secret: string | undefined): void {
  const mark = (req: IncomingMessage): void => {
    const peer = vouchedPeer(req, secret);
    delete req.headers[SECRET_HEADER];
    delete req.headers[PEER_HEADER];
    if (peer) (req as { [FRONT_PEER]?: string })[FRONT_PEER] = peer;
  };
  server.prependListener("request", mark);
  server.prependListener("upgrade", mark);
}
