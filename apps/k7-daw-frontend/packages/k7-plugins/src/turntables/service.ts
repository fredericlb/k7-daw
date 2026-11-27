// service.ts — what the desk reads of the account's file service, the one the project is opened through:
// the workspace's root, its audio files, a record's envelope. A record is a workspace file, and its path
// is in the same space as a clip's: the copy to the engine's machine and the tunnel's mapping carry both.
// See ../docs/hosting.md.

import { audioFilesOf } from "@studio/k7-wasm/records";
import { fget } from "@studio/project-shell/fileApi";
import { fileRoutes } from "@studio/project-shell/files";
import type { Envelope } from "./waveform";

export const service = {
  /** The workspace's absolute root, as the file service opened it. */
  root: async (): Promise<string> => (await fileRoutes.workspace()).root,

  /** Every audio file of the workspace, `stems/` folders included, since a record's stems are judged by them. */
  files: async (): Promise<Set<string>> =>
    new Set(audioFilesOf(await fileRoutes.tree(), true)),

  /** One record's envelope, transients and tempo — slow the first time, a read afterwards. */
  envelope: (path: string) =>
    fget<Envelope>(`/record/envelope?path=${encodeURIComponent(path)}`),
};

/** A record as the absolute path the engine is sent: the root, then `/`, then the workspace path. */
export function recordPath(root: string, rel: string): string {
  return `${root.replace(/[\\/]+$/, "")}/${rel}`;
}
