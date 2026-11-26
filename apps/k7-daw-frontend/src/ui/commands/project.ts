// project.ts — the bare-named project verbs the shell answers itself, because each writes
// through a ROUTE or from state only this browser holds: the undo journal, the master meter's
// fix, the mixdown and the cut, the k7 publish and the share link, an import.

import { fetchScene } from "@studio/k7-plugins/video";
import { runRedo, runUndo } from "@studio/project-shell/undo";
import { EXPORT_USAGE, parseExportAsk } from "../../exportArgs";
import { exportHost } from "../../exportHost";
import { resolveEntry } from "../../localTargets";
import { m } from "../../paraglide/messages.js";
import type { LocalCommand, LocalCtx } from "./context";
import { refuseWhileLocked } from "./locked";

/** The folder of a background film, asked BEFORE it starts; the server's verb writes there when it ends. */
async function askFilmFolder(ctx: LocalCtx, rest: string): Promise<void> {
  try {
    const folder = await exportHost().pickFolder();
    if (!folder) return ctx.setStatus(m["share.cancelled"]());
    // The front writes the film itself, so the folder must be a path it can reach.
    if (!folder.path || folder.path.includes('"'))
      return ctx.setStatus(m["share.filmNoPath"]());
    await ctx.runCommand(`picture render ${rest} into="${folder.path}"`.trim());
  } catch (e) {
    ctx.setStatus(m["share.failed"]({ reason: (e as Error).message }));
  }
}

export const PROJECT_COMMANDS: LocalCommand[] = [
  {
    // The verb is the server's; the shell only adds the folder the person picks, where a picker exists.
    name: "picture render",
    listed: false,
    match: "names",
    run: (ctx, rest) => {
      if (!exportHost().canPick || /(^|\s)into=/.test(rest)) return false;
      void askFilmFolder(ctx, rest.trim());
      return true;
    },
  },
  // Ctrl+Z / Ctrl+Y as commands: the project-wide journal, the editor's own buffer history staying with the keys.
  {
    name: "project undo",
    readable: {
      en: "Undo the last change",
      fr: "Annuler la dernière modification",
    },
    run: (ctx) => {
      void runUndo(ctx.historyDeps);
      return true;
    },
  },
  {
    name: "project redo",
    readable: {
      en: "Redo the change you took back",
      fr: "Rétablir la modification annulée",
    },
    run: (ctx) => {
      void runRedo(ctx.historyDeps);
      return true;
    },
  },
  {
    // The Fix button as a verb; local, because the trim comes from a peak only this browser saw (docs/masterMeter.md).
    name: "project master fix",
    readable: {
      en: "Lower the output until the last pass fits",
      fr: "Baisser la sortie pour rentrer le dernier passage",
    },
    // Optional, and the blank is the ordinary answer: a number is for another tab, or a CLI.
    args: [
      {
        name: "peak",
        kind: "number",
        optional: true,
        hint: "the pass's peak in dB — the header meter's own when left blank",
      },
    ],
    match: "names",
    run: (ctx, rest) => {
      const arg = rest.trim();
      const given = arg ? Number(arg) : null;
      if (given !== null && !Number.isFinite(given)) {
        ctx.setStatus(m["master.notADb"]({ arg }));
        return true;
      }
      const pass = ctx.masterMeterRef.current?.pass();
      if (given === null && !pass?.offered) {
        ctx.setStatus(
          pass ? m["master.nothingToFix"]() : m["master.noMeter"](),
        );
        return true;
      }
      ctx.masterMeterRef.current?.take();
      void ctx.fixClipping(given ?? (pass?.peak as number));
      return true;
    },
  },
  {
    // Which HALF of the project leaves is the first word — see exportArgs.ts.
    name: "project export",
    readable: {
      en: "Download the mix, picture, cut or tracks zip",
      fr: "Télécharger mixage, image, montage ou zip des pistes",
    },
    args: [
      {
        name: "what",
        kind: "choice",
        hint: "audio · video · timeline · tracks",
      },
      {
        name: "format",
        kind: "choice",
        hint: "wav · mp3 · webm · mp4 · otio · edl; for tracks, the timeline",
      },
      {
        name: "sound",
        kind: "choice",
        hint: "the video's audio — wav · mp3 · none; for audio, the timeline — main by default",
      },
    ],
    match: "names",
    run: (ctx, rest) => {
      const ask = parseExportAsk(rest);
      const name = ctx.projectDoc?.name;
      if (!ask) ctx.setStatus(m["status.usage"]({ syntax: EXPORT_USAGE }));
      else if (ask.kind === "audio")
        void ctx.share.exportMix(ask.format, ask.timeline);
      else if (ask.kind === "tracks") void ctx.share.exportTracks(ask.timeline);
      else if (ask.kind === "timeline")
        void ctx.share.exportTimeline(ask.format, ask.lane, name ?? "cut");
      else {
        const silent = ctx.videoDoc?.sound === null;
        if (silent && ask.audio) ctx.setStatus(m["status.videoSilent"]());
        const scene = ctx.videoDoc
          ? fetchScene(null, "export").catch(() => null)
          : Promise.resolve(null);
        void scene.then((s) =>
          ctx.share.exportVideo(
            s,
            ask.format,
            // The sound is what RENDER's audio input carries; an older backend says nothing and means the mix.
            silent ? null : ask.audio,
            name ?? "video",
            ask.settings,
          ),
        );
      }
      return true;
    },
  },
  {
    // The dialog reports what the translation did and then offers to open the result.
    name: "project import",
    readable: {
      en: "Import a .dawproject, an .als or an Ableton .zip",
      fr: "Importer un .dawproject, un .als ou un .zip Ableton",
    },
    args: [
      {
        name: "path",
        kind: "file",
        hint: "a .dawproject, .als or Ableton .zip of this workspace",
      },
    ],
    match: "names",
    run: (ctx, rest) => {
      const arg = rest.trim();
      const path = arg
        ? resolveEntry(arg, ctx.editorRef.current?.entries() ?? [])
        : null;
      if (refuseWhileLocked(ctx, "project import")) return true;
      if (!path) ctx.setStatus(m["status.importUsage"]());
      else ctx.imp.askImport(path);
      return true;
    },
  },
];
