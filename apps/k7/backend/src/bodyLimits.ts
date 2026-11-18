// bodyLimits.ts — the one JSON parser every route shares, mounted before any guard, so its limit
// is what an UNAUTHENTICATED caller can make this process parse. Small: no route left here takes
// more, the file hub's streamed paths and AI organize's 2 MB being the Rust front's. See ../docs/hardening.md.

import express, { type RequestHandler } from "express";

/** What any route's JSON body may weigh. */
const DEFAULT_JSON_LIMIT = "256kb";

/** The shared parser, at the default limit. */
export function jsonBodies(): RequestHandler {
  return express.json({ limit: DEFAULT_JSON_LIMIT });
}
