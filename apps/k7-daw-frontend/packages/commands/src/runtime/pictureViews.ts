// pictureViews.ts — the picture views the page reads, and the scene a picture render posts, computed over the Vfs by the picture loader.
// What a reader gets wrong: a compile's throw is an ANSWER (`error`), not a failure of the call; only a missing file is a retry.

import { hasPicture } from "@studio/song/surfaces";
import { sceneFrom } from "@studio/song-video/sceneFrom";
import { loadVideoProject } from "@studio/song-video/videoProject";
import {
  emptyVideoProject,
  webVideoProjectOf,
} from "@studio/song-video/webVideo";
import type { WebVideoProject } from "@studio/web-protocol";
import { relativeTo } from "./bundle.ts";
import { NeedsFiles } from "./vfs.ts";

export type SceneAnswer =
  | { scene: ReturnType<typeof sceneFrom>; mixes: boolean }
  | { error: string };
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

/** The scene a picture render posts, every media URL naming its file by its virtual path, and whether the mix is its sound. */
export function pictureSceneOf(project: string): SceneAnswer {
  if (!hasPicture(project))
    return { error: "this project draws no picture — nothing to render" };
  try {
    const web = webVideoProjectOf(loadVideoProject(project), (abs) => abs);
    if (!web.videoTracks.length)
      return { error: "this project draws no picture — nothing to render" };
    const scene = sceneFrom(
      web,
      null,
      "export",
      (p: string) => `media/${encodeURIComponent(p)}`,
    );
    return { scene, mixes: web.sound === "mix" };
  } catch (e) {
    return { error: refusal(e) };
  }
}
