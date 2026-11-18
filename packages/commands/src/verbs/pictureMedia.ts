// pictureMedia.ts — `picture prepare <lane>`: each shot's video made fit for the picture. A probe
// says what it needs — nothing, a stream copy, a full-resolution optimized copy, a small proxy —
// and the host makes the copies, in the BACKGROUND where it can, since a transcode takes minutes.
// Each copy lands beside its source and is named on the clip. See packages/song-video/docs/media.md.

import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { readYamlEntities } from "@studio/song/loader";
import type { RawClip } from "@studio/song/rawDoc";
import { VIDEO_SOURCE } from "@studio/song-schema/yaml/entitySchemas";
import {
  ANDROID_PLAYABLE_CODECS,
  PLAYABLE_CODECS,
  planVideo,
} from "@studio/video-prep/plan";
import type { CommandHost, SidHandler } from "../commands.ts";
import { isNoFfmpeg, NoFfmpegError, type PrepareAsk } from "../hostTypes.ts";
import { laneFile, placementsOf } from "./pictureShared.ts";
import { splitWords } from "./shared.ts";

/** What a browser plays as it is, so a machine with no ffmpeg needs do nothing for it. */
const BROWSER_PLAYS = /\.(mp4|m4v|webm)$/i;

/** The copies one clip still needs, as the ask that makes them, or null when it needs none. */
async function clipWork(
  ref: string,
  clip: RawClip,
  h: CommandHost,
): Promise<{
  say: string;
  ask: PrepareAsk | null;
  /** It needs a copy this host cannot make. */
  unconverted?: true;
}> {
  const source = String(clip.source);
  const abs = resolve(h.songPath, source);
  // A host with no reader says what the missing tool would: nothing can be said about the file.
  if (!h.probe) throw new NoFfmpegError("ffprobe", "this host reads no video.");
  const facts = await h.probe(abs);
  if (!facts) return { say: `${ref}: ${source} has no picture`, ask: null };
  const plan = planVideo(
    facts,
    process.platform === "android" ? ANDROID_PLAYABLE_CODECS : PLAYABLE_CODECS,
  );
  const have = (rel: string | undefined) =>
    !!rel && existsSync(resolve(h.songPath, rel));
  const wantOptimized = plan.optimized !== "none" && !have(clip.optimized);
  const wantProxy = plan.proxy && !have(clip.proxy);
  // Read without ffmpeg, nothing can be made: a proxy is a comfort the source stands in for, a copy is not.
  if (facts.noFfmpeg && wantOptimized)
    return {
      say: `${ref}: ${facts.codec || "an unknown codec"} in ${facts.pixFmt || "an unknown format"}, ${facts.width}×${facts.height}`,
      ask: null,
      unconverted: true,
    };
  if (facts.noFfmpeg && wantProxy)
    return {
      say: `${ref}: previewed from its source — no ffmpeg here for a proxy`,
      ask: null,
    };
  if (!wantOptimized && !wantProxy)
    return {
      say: `${ref}: ${plan.optimized === "none" && !plan.proxy ? "ready as it is" : "already prepared"}`,
      ask: null,
    };
  const optimized = wantOptimized
    ? plan.optimized === "remux"
      ? "remux"
      : "transcode"
    : null;
  return {
    say: `${ref}: ${plan.reason}`,
    ask: { at: h.songPath, clip: ref, source, optimized, proxy: wantProxy },
  };
}

export const PICTURE_MEDIA_VERBS: Record<string, SidHandler> = {
  "picture.prepare": async (a, h) => {
    const laneArg = splitWords(a)[0];
    if (!laneArg) {
      h.refuse(["usage: picture prepare <lane>  (e.g. picture prepare main)"]);
      return;
    }
    const found = laneFile(laneArg, h);
    if (!found) return;
    const clips = readYamlEntities(h.songPath, "clips").values as Record<
      string,
      RawClip
    >;
    const refs = [
      ...new Set(
        placementsOf(found)
          .map((p) => p.ref)
          .filter(Boolean),
      ),
    ];
    const lines: string[] = [];
    // Clips that may not play here and that no ffmpeg can convert — said once, in words, after the rest.
    const unconverted: string[] = [];
    for (const ref of refs) {
      const clip = clips[ref];
      // A still or a missing clip has nothing to transcode; `picture add` already reported the missing one.
      if (!clip || !VIDEO_SOURCE.test(String(clip.source ?? ""))) continue;
      let job: Awaited<ReturnType<typeof clipWork>>;
      try {
        job = await clipWork(ref, clip, h);
      } catch (e) {
        // No ffmpeg: a file a browser plays as it is needs nothing, and one bad file must not stop the others.
        if (isNoFfmpeg(e)) {
          if (!BROWSER_PLAYS.test(String(clip.source))) unconverted.push(ref);
          continue;
        }
        lines.push(`${ref}: could not read it — ${(e as Error).message}`);
        continue;
      }
      lines.push(job.say);
      if (job.unconverted) unconverted.push(ref);
      if (!job.ask) continue;
      if (!h.prepare) {
        lines.push(`${ref}: this host makes no copies`);
        continue;
      }
      // The host runs it in the background where it can: the request returns, and the copy shows up when it is written.
      try {
        const made = await h.prepare(job.ask);
        lines.push(
          made.line ??
            (made.started
              ? `${ref}: preparing in the background — the lane picks it up when it is written`
              : `${ref}: already being prepared`),
        );
      } catch (e) {
        lines.push(`${ref}: could not prepare it — ${(e as Error).message}`);
      }
    }
    if (unconverted.length)
      lines.push(
        `${unconverted.join(", ")}: this video may not play in the browser (an iPhone .mov, say) — convert it to MP4 (H.264), or install ffmpeg so k7 converts it for you`,
      );
    h.log(lines.length ? lines : [`nothing on ${laneArg} to prepare`]);
    return h.reload();
  },
};
