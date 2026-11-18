// pictureRender.ts — `picture render [webm|mp4] [wav|none] [size= quality= from= to=]`: the picture
// rendered on the SERVER, with no browser left open — in the background, since it takes as long
// as the song or longer, and into `exports/`. The words are the browser export's own, read by the
// one parser both use. The scene is built HERE, as the browser's export builds it, and posted to the host.
// See apps/k7/backend-rs/docs/file-engine-jobs.md.

import { hasPicture } from "@studio/song/surfaces";
import {
  parseVideoSettings,
  VIDEO_FORMATS,
  type VideoFormat,
} from "@studio/song-schema/renderSettings";
import { sceneFrom } from "@studio/song-video/sceneFrom";
import { loadVideoProject } from "@studio/song-video/videoProject";
import { webVideoProjectOf } from "@studio/song-video/webVideo";
import type { SidHandler } from "../commands.ts";
import { splitWords } from "./shared.ts";

export const PICTURE_RENDER_VERBS: Record<string, SidHandler> = {
  "picture.render": async (a, h) => {
    const words = splitWords(a).map((w) => w.toLowerCase());
    const format = (VIDEO_FORMATS as readonly string[]).includes(words[0] ?? "")
      ? (words.shift() as VideoFormat)
      : "webm";
    // The sound is the engine's mixdown, as the browser's export takes it; `none` makes a silent film.
    const audio =
      words[0] === "none"
        ? (words.shift(), false)
        : words[0] === "wav"
          ? (words.shift(), true)
          : true;
    const settings = parseVideoSettings(words);
    if (!settings) {
      h.refuse([
        "usage: picture render [webm|mp4] [wav|none] [size=720p|1280x720] [quality=low|medium|high|max] [from=<t>] [to=<t>]",
      ]);
      return;
    }
    if (!h.renderPicture) {
      h.refuse([
        "this host renders no picture — run `project export video` in the browser instead",
      ]);
      return;
    }
    const render = h.renderPicture;
    let doc: ReturnType<typeof loadVideoProject>;
    try {
      if (!hasPicture(h.songPath)) throw new Error("no picture");
      doc = loadVideoProject(h.songPath);
    } catch {
      h.refuse(["this project draws no picture — nothing to render"]);
      return;
    }
    if (!doc.videoTracks.length) {
      h.refuse(["this project draws no picture — nothing to render"]);
      return;
    }
    const web = webVideoProjectOf(doc, (abs) => abs);
    // Every URL names the file by its path; the host decides what of it the render page may fetch.
    const scene = sceneFrom(
      web,
      null,
      "export",
      (p: string) => `media/${encodeURIComponent(p)}`,
    );
    // The sound is what RENDER's audio input carries: unwired, the video is silent (song docs/videoOutputs.md).
    const mixes = audio && web.sound === "mix";
    const work = async () => {
      const rel = await render({ scene, format, settings, audio: mixes });
      return `picture rendered: ${rel}`;
    };
    if (h.background) {
      if (!h.background("picture render", work)) {
        h.refuse([
          "a picture render is already running — it writes into exports/ when it ends",
        ]);
        return;
      }
      h.log([
        `rendering the picture in the background (${format}${audio ? "" : ", silent"}) — it lands in exports/`,
      ]);
      return;
    }
    h.log([await work()]);
  },
};
