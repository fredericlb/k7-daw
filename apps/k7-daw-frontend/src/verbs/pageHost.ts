// pageHost.ts — the one page host, over the file API's routes, registered with the verbs' seam at boot.
// What a reader gets wrong: the palette's catalogue is fetched on first use, not at boot, since the first paint needs none of it.

import { fget, fsend } from "@studio/project-shell/fileApi";
import { setVerbsHost } from "@studio/project-shell/verbsHost";
import type { CommandInfo } from "@studio/web-protocol";
import { trackProcess } from "../processes";
import { createPageVerbs } from "./pageVerbs.ts";

export const pageVerbs = createPageVerbs({
  send: (path, body) => fsend("POST", path, body),
  read: (path, body) => fsend("POST", path, body, { fence: false }),
  catalogue: () => fget<CommandInfo[]>("/verb/catalogue"),
  track: trackProcess,
});

/** Registers the page's host with the file API's seam. */
export function installPageVerbs(): void {
  setVerbsHost(pageVerbs);
}
