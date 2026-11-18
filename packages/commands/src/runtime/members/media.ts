// media.ts — the media family of host members in the Worker: what a film is, its sound, its copies and its words (3f).
// What a reader gets wrong: the sound enters the Vfs as an adopt at the path the verb names, never as bytes through a body.

import type { CommandHost } from "../../commands.ts";
import type { SoundFacts, VideoFacts } from "../../hostTypes.ts";
import type { Vfs } from "../vfs.ts";
import {
  adoptFiles,
  type Hosting,
  hostCall,
  hostJob,
  type JobFile,
} from "./jobs.ts";

type MediaMembers = Pick<
  CommandHost,
  | "probe"
  | "extractSound"
  | "clipEnvelope"
  | "prepare"
  | "transcribe"
  | "renderPicture"
>;

/** The members of one run; each marks the run `hostCalled`. */
export function mediaMembers(h: Hosting, vfs: Vfs): MediaMembers {
  return {
    probe: async (file) =>
      (await hostCall<{ facts: VideoFacts | null }>(h, "probe", { file }))
        .facts,
    extractSound: async (video, out) => {
      const { job, value } = await hostJob<{
        facts: SoundFacts | null;
        files: JobFile[];
      }>(h, "extract", { video });
      adoptFiles(vfs, job, value.files, () => out);
      return value.facts;
    },
    // The copies are made and landed in the clip file by the server, in the background; the run only asks.
    prepare: async (ask) => ({
      started: (
        await hostCall<{ started: boolean }>(h, "prepare", {
          clip: ask.clip,
          source: ask.source,
          optimized: ask.optimized,
          proxy: ask.proxy,
        })
      ).started,
    }),
    // The film is rendered and written into `exports/` by the server, which outlives the page; the run waits for its path.
    renderPicture: async ({ scene, format, settings, audio }) =>
      (
        await hostJob<{ rel: string }>(h, "picture", {
          scene,
          format,
          settings,
          audio,
        })
      ).value.rel,
    transcribe: async (video) =>
      (await hostJob<{ srt: string | null }>(h, "transcribe", { video })).value
        .srt,
    clipEnvelope: async (clip) => {
      h.flags.hostCalled = true;
      const wf = await h.remote.envelope?.(clip).catch(() => null);
      return wf?.rms.length ? { peak: wf.rms, seconds: wf.seconds } : null;
    },
  };
}

/** Where a take is filed: the engine wrote it under the project's record directory, the page its picture under the project. */
export function takeMember(
  h: Hosting,
  recordDir: () => string,
  songDir: string,
): Pick<CommandHost, "saveTake"> {
  return {
    saveTake: async ({ file, video }) => {
      const picture =
        video === undefined
          ? undefined
          : video.startsWith("/")
            ? video
            : `${songDir.replace(/\/$/, "")}/${video}`;
      const { path } = await hostCall<{ path: string }>(h, "take/save", {
        recordDir: recordDir(),
        ...(file ? { file } : {}),
        ...(picture ? { video: picture } : {}),
      });
      return path;
    },
  };
}
