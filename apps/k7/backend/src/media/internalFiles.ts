// internalFiles.ts — what the Rust front asks of this backend under INTERNAL_FILES_PATH, behind `internalGate`:
// whether a bearer verifies, and the handshake by which it takes the file hub, answered `runs: false` since this
// backend runs none. The contract is
// apps/k7/backend-rs/docs/files.md, "The switch".

import express, { Router } from "express";
import { z } from "zod";

const tokenBody = z.object({ token: z.string().min(1).max(4096) });

/** What the seam's file half needs of this process. */
export interface InternalFilesDeps {
  /** The bearer's subject, or null: the same check the `/api` guard runs. */
  verifyToken: (token: string) => string | null;
}

export function internalFilesRouter(deps: InternalFilesDeps): Router {
  const r = Router();
  r.use(express.json({ limit: "8kb" }));

  // The token travels in the body, never the query string, so no access log keeps it.
  r.post("/verify-token", (req, res) => {
    const p = tokenBody.safeParse(req.body);
    if (!p.success)
      return void res.status(400).json({ error: "a token is required" });
    res.json({ sub: deps.verifyToken(p.data.token) });
  });

  // The front asks before it takes the family; an older backend's 404 means it still runs it.
  r.get("/hub", (_req, res) => {
    res.json({ runs: false });
  });

  return r;
}
