// useVideoEdits.ts — commit a finished picture drag to the lane's file, or its timeline's. Shared by the shell's
// timeline pane and by a timeline surface alone on a second device, which drag the same clips.
// See ../../docs/picture.md.

import { parse } from "@studio/k7-wasm/k7x";
import { timelineLaneOf } from "@studio/k7-wasm/schema";
import {
  editVideo,
  listVideoPlacements,
  type VideoOp,
} from "@studio/k7-wasm/text";
import { useCallback } from "react";
import { api } from "../api";
import { projectDir } from "../projectPaths";
import type { VideoEdit } from "../videoDrag";
import type { SetStatus } from "./useLogs";

/** A file and the path it was read at, kept together so the write lands where the read came from. */
const readAt = async (path: string) => ({
  path,
  content: (await api.read(path)).content,
});

/** Apply a finished picture drag to the lane's own file: values set by INDEX in one document, written once, and not a COMMAND. */
export function useVideoEdits(
  project: string,
  bumpReload: () => void,
  setStatus: SetStatus,
): (trackId: string, edits: VideoEdit[]) => Promise<void> {
  return useCallback(
    async (trackId: string, edits: VideoEdit[]) => {
      const dir = projectDir(project);
      // A timeline's lane, `<timeline>><lane>`, is a map in that timeline's file (song/docs/timelines.md).
      const tl = timelineLaneOf(trackId);
      const inProject = (file: string) => `${dir ? `${dir}/` : ""}${file}`;
      try {
        // A lane's file is `nodes/`, or a chain project's `tracks/` (apps/k7-daw-frontend/packages/song/docs/chainFiles.md).
        const { path, content } = tl
          ? await readAt(inProject(`timelines/${tl.timeline}.k7x`))
          : await readAt(inProject(`nodes/${trackId}.k7x`)).catch(() =>
              readAt(inProject(`tracks/${trackId}.k7x`)),
            );
        const pictures = tl
          ? (parse(content) as { pictures?: { lane?: unknown }[] } | null)
          : null;
        const found = tl
          ? (pictures?.pictures ?? []).findIndex((p) => p?.lane === tl.lane)
          : 0;
        if (found < 0) return; // the lane went away under the drag — nothing to write
        const at = tl ? ["pictures", found] : [];
        const before = listVideoPlacements(content, at);
        const ops: VideoOp[] = [];
        for (const e of edits) {
          ops.push({
            op: "setFieldAt",
            index: e.index,
            field: e.field,
            value: e.value,
          });
          // A head trim moves the in-point, and the keys go with it so they stay on their frames (song-video docs/animation.md).
          const was = before[e.index];
          if (e.field === "from" && was)
            ops.push({
              op: "shiftAnimationAt",
              index: e.index,
              delta: e.value - was.sourceFrom,
            });
        }
        const next = editVideo(content, ops, at).text;
        if (next === content) return; // no such placement — nothing to write, nothing to say
        await api.write(path, next);
        bumpReload();
      } catch (err) {
        setStatus((err as Error).message, "error");
      }
    },
    [project, bumpReload, setStatus],
  );
}
