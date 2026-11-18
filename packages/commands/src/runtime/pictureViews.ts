// pictureViews.ts — the two picture views the page reads, computed over the Vfs by the same loader the picture verbs use.
// What a reader gets wrong: a compile's throw is an ANSWER (`error`), not a failure of the call; only a missing file is a retry.

import { hasPicture } from "@studio/song/surfaces";
import { loadVideoProject } from "@studio/song-video/videoProject";
import {
  emptyVideoProject,
  webVideoProjectOf,
} from "@studio/song-video/webVideo";
import type { WebVideoProject } from "@studio/web-protocol";
import { relativeTo } from "./bundle.ts";
import { NeedsFiles } from "./vfs.ts";

export type VideoDocAnswer = { doc: WebVideoProject } | { error: string };
export interface VideoErrorsAnswer {
  errors: string[];
  diagnostics: never[];
}

/** A throw that is the compile's own refusal; a missing file must reach the runtime's fetch loop. */
const refusal = (e: unknown): string => {
  if (e instanceof NeedsFiles) throw e;
  return (e as Error).message;
};

/** The project's picture, EMPTY when it has none; every media path workspace-relative, as the page reads them. */
export function videoDocOf(
  project: string,
  workspaceRoot: string,
): VideoDocAnswer {
  if (!hasPicture(project)) return { doc: emptyVideoProject(project) };
  try {
    return {
      doc: webVideoProjectOf(
        loadVideoProject(project),
        (abs) => relativeTo(workspaceRoot, abs) ?? abs,
      ),
    };
  } catch (e) {
    return { error: refusal(e) };
  }
}

/** Everything wrong with the picture: its warnings, or the throw of a picture that cannot be built. */
export function videoErrorsOf(project: string): VideoErrorsAnswer {
  if (!hasPicture(project)) return { errors: [], diagnostics: [] };
  try {
    return { errors: loadVideoProject(project).warnings, diagnostics: [] };
  } catch (e) {
    return { errors: [refusal(e)], diagnostics: [] };
  }
}
