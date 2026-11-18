// mount.ts — what k7's backend still mounts beside the front: the bearer scheme, which guards every `/api` route
// after it, and the internal seam's file half (./internalFiles.ts). The shares and the file hub are the Rust
// front's (apps/k7/backend-rs/docs/files.md, mediatek/shares.md); it calls `verify-token` here for a bearer.
// It mounts no auth and no CORS: server.ts applies both to the whole process before calling this.
// A token is valid only while listed in its store under the auth dir (../authState.ts).

import { join } from "node:path";
import { apiTokenAuth, fileTokenStore } from "@studio/oidc/api-auth";
import type { Express, RequestHandler, Router } from "express";
import { authDir } from "../authState.ts";
import { internalFilesRouter } from "./internalFiles.ts";

/** The library's bearer scheme, created on its own so a mount BEFORE this one can be handed a token. */
export function mediaTokens(): ReturnType<typeof apiTokenAuth> {
  // A bearer stands in for the session: an account's file service calling the médiathèque has no cookie jar.
  return apiTokenAuth(
    process.env.API_TOKEN_SECRET || `${process.env.SESSION_SECRET ?? ""}:pl8`,
    { store: fileTokenStore(join(authDir(), "library-tokens.json")) },
  );
}

/** Mount the seam's file half and the `/api` guard on `app`; call it after setupAuth and before the preferences. */
export function mountMedia(
  app: Express,
  opts: {
    requireAuth: RequestHandler;
    tokens: ReturnType<typeof mediaTokens>;
    /** The internal seam's router (packages/engine-registry/docs/internalSeam.md). */
    internal: Router;
  },
): void {
  const { requireAuth, tokens } = opts;
  // What the Rust side asks of this backend, behind the seam's gate.
  opts.internal.use(
    "/files",
    internalFilesRouter({ verifyToken: tokens.verifyApiToken }),
  );
  opts.internal.use((_req, res) => {
    res.status(404).json({ error: "no such internal route" });
  });
  // Guards every `/api` route mounted after it, the preferences among them: a bearer stands in for the session.
  app.use("/api", tokens.bearerOrSession(requireAuth));
}
