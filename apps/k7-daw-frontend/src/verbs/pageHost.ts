// pageHost.ts — the one page host, made on first use over the real Worker and the file API's session.
// What a reader gets wrong: nothing here starts a Worker at import; the first picture view does, through the spawner `installPageVerbs` was given.

import { cfg, fget, fileHeaders, fsend } from "@studio/project-shell/fileApi";
import { setVerbsHost } from "@studio/project-shell/verbsHost";
import type { CommandInfo } from "@studio/web-protocol";
import { trackProcess } from "../processes";
import { createPageVerbs, fetchActiveProject } from "./pageVerbs.ts";
import { createVerbs, type WorkerLike } from "./verbsWorker.ts";

let spawner: (() => WorkerLike) | undefined;

const verbs = createVerbs({
  spawn: () => {
    if (!spawner) throw new Error("the verbs' Worker is not installed");
    return spawner();
  },
  base: () => cfg().base,
  headers: () => ({ ...(fileHeaders() as Record<string, string>) }),
});

export const pageVerbs = createPageVerbs({
  verbs,
  send: (path, body) => fsend("POST", path, body),
  // The crate builds the catalogue, so it is asked with the first palette, not at boot.
  catalogue: () => fget<CommandInfo[]>("/verb/catalogue"),
  session: () => {
    const c = cfg();
    return { key: `${c.base}|${c.workspace}|${c.engineToken}` };
  },
  activeProject: fetchActiveProject,
  track: trackProcess,
});

/** Registers the page's host with the file API's seam; `spawn` makes the real Worker (kept out of this file so a test needs no build of it). */
export function installPageVerbs(spawn: () => WorkerLike): void {
  spawner = spawn;
  setVerbsHost(pageVerbs);
}
