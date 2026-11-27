// paths.ts — the single definition of the state directory's layout, which the C++ engine,
// the file service, the supervisor and the desktop app all read or write by name. Where the
// directory IS: studioDir() below. USE THESE, never a hand-joined path: a wrong
// path doesn't throw, it reads as an empty file, so a second spelling diverges silently.
// See docs/paths.md.

import { homedir } from "node:os";
import { join, resolve } from "node:path";

/** The state dir — `~/.k7`, or `$STUDIO_HOME` when set; normalised, and the same on all three platforms. */
export function studioDir(): string {
  return resolve(process.env.STUDIO_HOME || join(homedir(), ".k7"));
}

/** An email as `_`-separated; it names the SSH key dir and the SESSION, no longer a workspace root — docs/paths.md. */
export function slugify(email: string): string {
  return email.replace(/[^a-zA-Z0-9]/g, "_");
}

/** One file per hosted SERVICE, a directory rather than one list because three backends booting together would lose an entry from it. Read for pl8's address by the file service (`mediatekLink.ts`); the supervisor does NOT connect the engine to what is here. */
export function webEndpointsDir(): string {
  return join(studioDir(), "web-endpoints");
}

/** `<state>/workspace` — the workspaces the file service owns, one level, no user beneath it. */
export function rootDir(): string {
  return join(studioDir(), "workspace");
}
