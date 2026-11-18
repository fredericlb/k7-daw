// nodeMedia.ts — the media members of a host on a machine that has the tools: probe, extract, prepare, playlist stems.
// Server and test hosts only: the browser's Worker asks these over HTTP, so no verb file may import this one.

import { existsSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { extractSound } from "@studio/audio-analysis/extract";
import { coldOutDir, runColdPasses } from "@studio/audio-passes/coldPass";
import { findEngineBinary } from "@studio/engine-client/engineBinary";
import { readYamlEntities } from "@studio/song/loader";
import type { RawClip } from "@studio/song/rawDoc";
import { setFileValue } from "@studio/song/yaml/edit";
import { wsExists } from "@studio/song/yaml/workspaceFile";
import { isAudio } from "@studio/song-schema/audioFiles";
import { recordPathOf } from "@studio/song-schema/records";
import { copyPathOf } from "@studio/video-prep/copyPath";
import { probeVideo } from "@studio/video-prep/probe";
import { makeCopy } from "@studio/video-prep/transcode";
import type { CommandHost } from "./commands.ts";
import type { PlaylistStemsSong, PrepareAsk } from "./hostTypes.ts";

/** What the members read from their host, late: a test host is finished after they are made. */
export interface MediaNeeds {
  background?: () => CommandHost["background"];
  runColdStep?: () => CommandHost["runColdStep"];
}

/** One clip's copies, made in order: the optimized one, then the proxy from whatever decodes best. */
async function prepareWork(ask: PrepareAsk): Promise<string> {
  const abs = resolve(ask.at, ask.source);
  const file = resolve(ask.at, "clips", `${ask.clip}.k7x`);
  const clip = readYamlEntities(ask.at, "clips").values[ask.clip] as RawClip;
  const have = (rel: string | undefined) =>
    !!rel && existsSync(resolve(ask.at, rel));
  // The proxy is made from whatever DECODES: the optimized copy when there is one, which is also the faster read.
  let decodable =
    clip?.optimized && have(clip.optimized) ? clip.optimized : ask.source;
  if (ask.optimized) {
    const rel = copyPathOf(ask.source, "optimized");
    await makeCopy(ask.optimized, abs, resolve(ask.at, rel));
    setFileValue(file, ["optimized"], rel);
    decodable = rel;
  }
  if (ask.proxy) {
    const rel = copyPathOf(ask.source, "proxy");
    await makeCopy("proxy", resolve(ask.at, decodable), resolve(ask.at, rel));
    setFileValue(file, ["proxy"], rel);
  }
  const made = [ask.optimized && "optimized", ask.proxy && "proxy"];
  return `${ask.clip}: prepared (${made.filter(Boolean).join(" + ")})`;
}

/** One record of a playlist: a workspace audio file, its stems made unless `stems/` beside it already holds them. */
async function stemsOfRecord(
  root: string,
  song: string,
  steps: Parameters<NonNullable<CommandHost["playlistStems"]>>[0]["steps"],
  run: NonNullable<CommandHost["runColdStep"]>,
): Promise<PlaylistStemsSong> {
  const rel = recordPathOf(song);
  if (!rel || !isAudio(rel)) return { song, state: "absent" };
  const file = join(root, ...rel.split("/"));
  if (!existsSync(file)) return { song, state: "absent" };
  // "Already done" is a question about the FOLDER, not a flag anybody keeps — docs/playlist.md.
  const out = coldOutDir(file);
  if (wsExists(out) && readdirSync(out).some(isAudio))
    return { song, state: "held" };
  try {
    const wrote = await runColdPasses([file], steps, run);
    return { song, state: "done", files: wrote.flatMap((d) => d.files) };
  } catch (e) {
    // Per record, and the loop goes on: one unreadable file must not cost the other twenty-nine.
    return { song, state: "failed", error: (e as Error).message };
  }
}

/** The members, over this machine's ffprobe, ffmpeg and decoders. */
export function nodeMediaMembers(
  needs: MediaNeeds = {},
): Pick<CommandHost, "probe" | "extractSound" | "prepare" | "playlistStems"> {
  return {
    probe: (file) => probeVideo(file),
    extractSound: (video, out) =>
      extractSound(video, out, { engineBin: findEngineBinary() }),
    prepare: async (ask) => {
      const background = needs.background?.();
      if (!background) return { started: true, line: await prepareWork(ask) };
      const tag = `picture prepare ${resolve(ask.at, ask.source)}`;
      return { started: background(tag, () => prepareWork(ask)) };
    },
    playlistStems: async ({ root, songs, steps }) => {
      const run = needs.runColdStep?.();
      if (!run) throw new Error("this host cannot run an offline step");
      const done: PlaylistStemsSong[] = [];
      for (const song of songs)
        done.push(await stemsOfRecord(root, song, steps, run));
      return { songs: done };
    },
  };
}
