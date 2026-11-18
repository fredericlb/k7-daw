// transports.ts — the two ways a screen reaches the git routes: through the account's file service (the home
// page, which has no project open) or through the open project's configured transport (the editor's bar).

import { cfg, fileHeaders } from "@studio/project-shell/fileApi";
import { type AccountFiles, filesHeaders } from "../projects/api";
import type { GitTransport } from "./gitApi";

export const accountTransport = (m: AccountFiles): GitTransport => ({
  base: m.fileBase,
  headers: () => filesHeaders(m),
});

/** The open project's file service, or null before one is configured. */
export function projectTransport(): GitTransport | null {
  try {
    return {
      base: cfg().base,
      headers: () => ({ ...fileHeaders() }) as Record<string, string>,
    };
  } catch {
    return null;
  }
}
