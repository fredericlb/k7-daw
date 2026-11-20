// render.ts — the render family of host members in the Worker: a doc bounced, a peak measured, a cut, a playlist's stems (3f, M5).
// What a reader gets wrong: a job's files enter the Vfs as adopts, never as bytes; the server writes the médiathèque's stems itself.

import type { CommandHost } from "../../commands.ts";
import type { ColdStep, PlaylistStemsSong, Renderer } from "../../hostTypes.ts";
import type { Vfs } from "../vfs.ts";
import {
  adoptFiles,
  type Hosting,
  hostCall,
  hostJob,
  type JobFile,
} from "./jobs.ts";

type RenderMembers = Pick<
  CommandHost,
  "render" | "peak" | "runColdStep" | "playlistStems"
>;

/** What a finished render answers: the engine's manifest and the files it left in the job's folder. */
interface Rendered {
  manifest: { mix: string; stems: { name: string; file: string }[] };
  files: JobFile[];
}

/** The members of one run: each marks the run `hostCalled`, and a job's files are adopted into `vfs`. */
export function renderMembers(h: Hosting, vfs: Vfs): RenderMembers {
  const render: Renderer = async (doc, outDir, opts) => {
    const { job, value } = await hostJob<Rendered>(h, "render", {
      doc,
      ...(opts?.stems ? { stems: true } : {}),
      ...(opts?.base ? { base: opts.base } : {}),
    });
    adoptFiles(vfs, job, value.files, (out) => `${outDir}/${out}`);
    return value.manifest;
  };
  return {
    render,
    peak: async (file) =>
      (await hostCall<{ db: number }>(h, "peak", { file })).db,
    runColdStep: async ({ step, source, outDir }) => {
      const { job, value } = await hostJob<{ files: JobFile[] }>(
        h,
        "separate",
        {
          source,
          model: step.model,
          outputs: step.outputs,
        },
      );
      adoptFiles(vfs, job, value.files, (out) => `${outDir}/${out}`);
      return value.files.map((f) => `${outDir}/${f.out}`);
    },
    playlistStems: async (ask: {
      root: string;
      songs: string[];
      steps: ColdStep[];
    }) => {
      const { value } = await hostJob<{ songs: PlaylistStemsSong[] }>(
        h,
        "playlist-stems",
        {
          songs: ask.songs,
          steps: ask.steps.map((s) => ({
            step: s.step,
            model: s.model,
            outputs: s.outputs,
          })),
        },
      );
      return value;
    },
  };
}
