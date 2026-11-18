// seam.ts — the paths, the gate and the shapes of the internal seam: the loopback HTTP API through which the
// TypeScript that stays reaches the registry, the tunnel hub and the machine's engine, all the Rust front's.
// The contract, to the byte, is docs/internalSeam.md: change it there first.

import { timingSafeEqual } from "node:crypto";
import { isLoopbackAddress, peerAddress } from "@studio/web-protocol/services";
import type { RequestHandler } from "express";
import type { EngineRole } from "./owners.ts";

/** Everything under it is the internal API, and nothing a page calls. */
export const INTERNAL_PATH = "/api/internal";
/** The registry, the hub and the local engine. */
export const INTERNAL_ENGINE_PATH = `${INTERNAL_PATH}/engine`;
/** What the backend's callers ask of the Rust file hub. */
export const INTERNAL_FILES_PATH = `${INTERNAL_PATH}/files`;
/** Carries the secret both processes hold: `K7_FRONT_SECRET`, or one drawn per process. */
export const INTERNAL_SECRET_HEADER = "x-k7-internal";

/** What one engine is to one account, as every caller of the seam reads it. */
export interface EngineView {
  id: string;
  hostname: string;
  /** `engineRole` with `server` false: the render machine is told by `renderer`. */
  role: EngineRole;
  /** The account holds a live claim on it. */
  holds: boolean;
  /** Its tunnel is up now. */
  tunnel: boolean;
  /** Its last tunnel presented the render key, up or not. */
  renderer: boolean;
  /** Its machine's performance score, null when it could not rate itself. */
  score: number | null;
}

const sameSecret = (a: string, b: string): boolean => {
  const x = Buffer.from(a);
  const y = Buffer.from(b);
  return x.length === y.length && timingSafeEqual(x, y);
};

/** Refuses a peer that is not this machine (403), then a request without the secret (401). */
export function internalGate(secret: string): RequestHandler {
  return (req, res, next) => {
    if (!isLoopbackAddress(peerAddress(req)))
      return void res
        .status(403)
        .json({ error: "the internal API answers this machine only" });
    const given = req.headers[INTERNAL_SECRET_HEADER];
    if (!secret || typeof given !== "string" || !sameSecret(given, secret))
      return void res
        .status(401)
        .json({ error: "the internal API needs this process's secret" });
    res.setHeader("Cache-Control", "no-store");
    next();
  };
}
