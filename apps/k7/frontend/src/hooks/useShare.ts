// useShare.ts — the export-panel entries that are not already CLI verbs: render a mixdown
// through the engine, a track zip, a video, a cut. Held here rather than in `SharePanel` so the panel and
// the command bar run one implementation — `project export` is those same entries, and a
// second copy behind the palette is how the two come to differ.

import { resolveVideoSettings } from "@studio/song-schema/renderSettings";
import type { VideoScene } from "@studio/ui-eyes/scene";
import { useCallback, useState } from "react";
import { api } from "../api";
import { downloadBlob as download, downloadLink } from "../downloadBlob";
import { awaitStart, newToken } from "../downloadSignal";

import type {
  MixFormat,
  TimelineFormat,
  VideoFormat,
  VideoSettings,
} from "../exportArgs";
import { m } from "../paraglide/messages.js";
import { trackProcess } from "../processes";

export interface Share {
  /** A render is running: the menu greys itself out rather than starting a second one. */
  busy: boolean;
  exportMix: (format: MixFormat, timeline?: string) => Promise<void>;
  /** Every track as a zip, handed to the browser as a link; the panel greys out while the render runs. */
  exportTracks: (timeline?: string) => Promise<void>;
  /** …and the PICTURE, rendered here rather than by the engine — with the mixdown married in
   *  unless `audio` is null. The scene is the caller's: only the viewport's host has one. */
  exportVideo: (
    scene: VideoScene | null,
    format: VideoFormat,
    audio: MixFormat | null,
    name: string,
    settings?: VideoSettings,
  ) => Promise<void>;
  /** …and the CUT, for another editor: `picture interchange` writes it, then it is downloaded (song-video docs/interchange.md). */
  exportTimeline: (
    project: string,
    format: TimelineFormat,
    lane: string | undefined,
    name: string,
    run: (line: string) => Promise<unknown>,
  ) => Promise<void>;
}

export function useShare(onStatus: (s: string) => void): Share {
  const [busy, setBusy] = useState(false);

  const exportMix = useCallback(
    async (format: MixFormat, timeline?: string) => {
      setBusy(true);
      const what = `${format.toUpperCase()} mixdown${timeline ? ` of ${timeline}` : ""}`;
      onStatus(`rendering ${what}…`);
      // A route, not a command, so the service lists nothing: this page lists it (docs/processes.md).
      const shown = trackProcess(`export ${what}`);
      try {
        const { blob, filename } = await api.exportMix(format, timeline);
        download(blob, filename);
        onStatus(`exported ${filename}`);
        shown.end();
      } catch (e) {
        onStatus(`export failed: ${(e as Error).message}`);
        shown.end((e as Error).message);
      } finally {
        setBusy(false);
      }
    },
    [onStatus],
  );

  const exportTracks = useCallback(
    async (timeline?: string) => {
      setBusy(true);
      onStatus(m["share.tracksRendering"]());
      const shown = trackProcess(
        `export tracks${timeline ? ` of ${timeline}` : ""}`,
      );
      try {
        const token = newToken();
        downloadLink(api.tracksUrl(token, timeline));
        // Held until the route's first byte, which sets the token's cookie, so a second click cannot start a second render.
        const started = await awaitStart(token);
        if (started.kind === "refused") {
          onStatus(m["share.tracksFailed"]({ reason: started.message }));
          shown.end(started.message);
        } else {
          onStatus(m["share.tracksStarted"]());
          shown.end();
        }
      } finally {
        setBusy(false);
      }
    },
    [onStatus],
  );

  const exportVideo = useCallback(
    async (
      scene: VideoScene | null,
      format: VideoFormat,
      audio: MixFormat | null,
      name: string,
      settings: VideoSettings = {},
    ) => {
      if (!scene) {
        onStatus("this project draws no picture — nothing to render");
        return;
      }
      setBusy(true);
      // Rendered in this page, so no poll reports it: this page lists it (docs/processes.md).
      const shown = trackProcess(`export ${format.toUpperCase()} video`);
      try {
        // The sound FIRST: the engine renders it, and a failure there should not cost a minute
        // of frames. Decoded here because the container wants neither a wav nor an mp3.
        let track: AudioBuffer | undefined;
        if (audio) {
          onStatus(`rendering ${audio.toUpperCase()} mixdown…`);
          shown.progress(null, `${audio.toUpperCase()} mixdown`);
          const { blob } = await api.exportMix(audio);
          const ctx = new AudioContext();
          try {
            track = await ctx.decodeAudioData(await blob.arrayBuffer());
          } finally {
            void ctx.close();
          }
        }
        // Resolved as the server render resolves them, so the two make the same file.
        const {
          from,
          frameCount: total,
          size,
        } = resolveVideoSettings(settings, scene);
        onStatus(`rendering ${total} frames…`);
        // Loaded here, on the export: the muxer behind it is the heaviest thing the editor could ship and the rarest it runs.
        const { exportScene } = await import("@studio/ui-eyes/export");
        const file = await exportScene(
          scene,
          {
            format,
            from,
            frameCount: total,
            ...(size ? { size } : {}),
            ...(settings.quality ? { quality: settings.quality } : {}),
            ...(track ? { audio: track } : {}),
          },
          // A frame count, not a percentage: a viewport export is slow and "1200 / 4500" is the
          // only progress that says how slow.
          (p) => {
            onStatus(`rendering ${p.frame} / ${p.total} frames…`);
            shown.progress(p.frame / p.total, `frame ${p.frame} / ${p.total}`);
          },
        );
        // Named by the container itself: mediabunny owns both strings, so the two cannot drift.
        download(
          new Blob([file.buffer], { type: file.mimeType }),
          `${name || "video"}${file.extension}`,
        );
        onStatus(`exported ${total} frames`);
        shown.end();
      } catch (e) {
        onStatus(`export failed: ${(e as Error).message}`);
        shown.end((e as Error).message);
      } finally {
        setBusy(false);
      }
    },
    [onStatus],
  );

  const exportTimeline = useCallback(
    async (
      project: string,
      format: TimelineFormat,
      lane: string | undefined,
      name: string,
      run: (line: string) => Promise<unknown>,
    ) => {
      setBusy(true);
      try {
        await run(`picture interchange ${format}${lane ? ` ${lane}` : ""}`);
        // The verb writes to one FIXED path, so nothing it said has to be parsed to find the file.
        const path = `${project === "." ? "" : `${project}/`}exports/cut.${format}`;
        const { content } = await api.read(path);
        download(
          new Blob([content], {
            type: format === "otio" ? "application/json" : "text/plain",
          }),
          `${name || "cut"}.${format}`,
        );
        onStatus(`exported ${name || "cut"}.${format}`);
      } catch (e) {
        onStatus(`export failed: ${(e as Error).message}`);
      } finally {
        setBusy(false);
      }
    },
    [onStatus],
  );

  return {
    busy,
    exportMix,
    exportTracks,
    exportVideo,
    exportTimeline,
  };
}
