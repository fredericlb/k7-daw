// scene.ts — the plain VideoScene @studio/ui-eyes draws, asked of the backend's one builder (`POST /picture/scene`).
// What a reader gets wrong: a scene is asked once per picture document and armed lane, and an unchanged answer keeps its object.

import { fsend, mediaPrefix } from "@studio/project-shell/fileApi";
import type { VideoScene } from "@studio/ui-eyes/scene";
import type { WebVideoProject } from "@studio/web-protocol";
import { useEffect, useRef, useState } from "react";

/** Who reads the scene: an EXPORT renders from the optimized copy or the source, never from a proxy (apps/k7-daw-wasm/verbs/docs/picture/media.md). */
export type SceneUse = "preview" | "export";

/** The scene of the open project, its files this file service's `/media` URLs; null when the picture does not build. */
export async function fetchScene(
  armed: string | null,
  use: SceneUse,
): Promise<VideoScene | null> {
  const a = await fsend<{ scene: VideoScene } | { error: string }>(
    "POST",
    "/picture/scene",
    { media: mediaPrefix(), use, ...(armed ? { armed } : {}) },
    { fence: false },
  );
  return "scene" in a ? a.scene : null;
}

/** The scene of `doc` as the viewport draws it: asked again when the document or the armed lane changes, kept while it answers alike. */
export function useScene(
  doc: WebVideoProject | null,
  /** The armed picture lane, which is what `monitor: auto` opens a lens for. */
  armed: string | null = null,
  use: SceneUse = "preview",
): { scene: VideoScene | null; settled: boolean } {
  const [state, setState] = useState<{
    scene: VideoScene | null;
    settled: boolean;
  }>({ scene: null, settled: false });
  // The last answer's text: a scene owns a GL context and a <video> per layer, so an unchanged one keeps its object.
  const last = useRef<string | null>(null);
  useEffect(() => {
    if (!doc) {
      last.current = null;
      setState({ scene: null, settled: true });
      return;
    }
    let alive = true;
    fetchScene(armed, use)
      .then((scene) => {
        if (!alive) return;
        const text = JSON.stringify(scene);
        setState((cur) =>
          text === last.current && cur.settled ? cur : { scene, settled: true },
        );
        last.current = text;
      })
      .catch(() => {
        // A blip keeps the picture on screen; the document's own read is what says it broke.
        if (alive) setState((cur) => ({ ...cur, settled: true }));
      });
    return () => {
      alive = false;
    };
  }, [doc, armed, use]);
  return state;
}
