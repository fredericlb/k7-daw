// useShare.ts — the export-panel entries: render a mixdown through the engine, a track zip, a video, a cut,
// and put the file where the person chose (exportHost.ts). Held here rather than in `SharePanel` so the panel and
// the command bar run one implementation — `project export` is those same entries. The folder is asked BEFORE the work.

import type { MixFormat, TimelineFormat } from "@studio/k7-wasm/edit";
import {
  resolveVideoSettings,
  type VideoFormat,
  type VideoSettings,
} from "@studio/k7-wasm/picture";
import type { VideoScene } from "@studio/ui-eyes/scene";
import { useCallback, useState } from "react";
import { api } from "../api";
import { downloadLink } from "../downloadBlob";
import { awaitStart, newToken } from "../downloadSignal";
import {
  type ExportHost,
  exportHost,
  type Folder,
  type Saved,
} from "../exportHost";
import { m } from "../paraglide/messages.js";
import { trackProcess } from "../processes";

export interface Share {
  /** A render is running: the menu greys itself out rather than starting a second one. */
  busy: boolean;
  /** The file the last export put in the person's folder, for the panel's "Show in folder" and "Share…". */
  last: Saved | null;
  host: ExportHost;
  reveal: () => Promise<void>;
  shareLast: () => Promise<void>;
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
  /** …and the CUT, for another editor: `picture interchange` says it and it is saved (song-video docs/interchange.md). */
  exportTimeline: (
    format: TimelineFormat,
    lane: string | undefined,
    name: string,
  ) => Promise<void>;
}

export function useShare(onStatus: (s: string) => void): Share {
  const [busy, setBusy] = useState(false);
  const [last, setLast] = useState<Saved | null>(null);
  const host = exportHost();

  /** The folder, asked before any work; null when the person cancelled. */
  const folderFor = useCallback(async (): Promise<Folder | null> => {
    try {
      const folder = await host.pickFolder();
      if (!folder) onStatus(m["share.cancelled"]());
      return folder;
    } catch (e) {
      onStatus(m["share.failed"]({ reason: (e as Error).message }));
      return null;
    }
  }, [host, onStatus]);

  /** Put `blob` in `folder` and say where it went. */
  const place = useCallback(
    async (folder: Folder, name: string, blob: Blob) => {
      const saved = await host.save(folder, name, blob);
      setLast(saved);
      onStatus(m["share.saved"]({ name: saved.name, folder: folder.name }));
    },
    [host, onStatus],
  );

  const exportMix = useCallback(
    async (format: MixFormat, timeline?: string) => {
      const folder = await folderFor();
      if (!folder) return;
      setBusy(true);
      const what = `${format.toUpperCase()} mixdown${timeline ? ` of ${timeline}` : ""}`;
      onStatus(`rendering ${what}…`);
      // A route, not a command, so the service lists nothing: this page lists it (docs/processes.md).
      const shown = trackProcess(`export ${what}`);
      try {
        const { blob, filename } = await api.exportMix(format, timeline);
        await place(folder, filename, blob);
        shown.end();
      } catch (e) {
        onStatus(m["share.failed"]({ reason: (e as Error).message }));
        shown.end((e as Error).message);
      } finally {
        setBusy(false);
      }
    },
    [folderFor, place, onStatus],
  );

  const exportTracks = useCallback(
    async (timeline?: string) => {
      const folder = await folderFor();
      if (!folder) return;
      setBusy(true);
      onStatus(m["share.tracksRendering"]());
      const shown = trackProcess(
        `export tracks${timeline ? ` of ${timeline}` : ""}`,
      );
      try {
        if (host.kind === "browser") {
          const token = newToken();
          downloadLink(api.tracksUrl(token, timeline));
          // Held until the route's first byte, which sets the token's cookie, so a second click cannot start a second render.
          const started = await awaitStart(token);
          if (started.kind === "refused") {
            onStatus(m["share.tracksFailed"]({ reason: started.message }));
            shown.end(started.message);
            return;
          }
          onStatus(m["share.tracksStarted"]());
        } else {
          const { blob, filename } = await api.exportTracks(timeline);
          await place(folder, filename, blob);
        }
        shown.end();
      } catch (e) {
        onStatus(m["share.tracksFailed"]({ reason: (e as Error).message }));
        shown.end((e as Error).message);
      } finally {
        setBusy(false);
      }
    },
    [folderFor, host, place, onStatus],
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
      const folder = await folderFor();
      if (!folder) return;
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
        await place(
          folder,
          `${name || "video"}${file.extension}`,
          new Blob([file.buffer], { type: file.mimeType }),
        );
        shown.end();
      } catch (e) {
        onStatus(m["share.failed"]({ reason: (e as Error).message }));
        shown.end((e as Error).message);
      } finally {
        setBusy(false);
      }
    },
    [folderFor, place, onStatus],
  );

  const exportTimeline = useCallback(
    async (format: TimelineFormat, lane: string | undefined, name: string) => {
      const folder = await folderFor();
      if (!folder) return;
      setBusy(true);
      try {
        // The verb SAYS the cut as its first output line and writes nothing; what it left out follows.
        const said = await api.command(
          `picture interchange ${format}${lane ? ` ${lane}` : ""}`,
          { want: [] },
        );
        const [text, ...left] = said.output;
        if (said.refused || text === undefined)
          throw new Error(said.output.join(" · ") || "nothing to hand over");
        await place(
          folder,
          `${name || "cut"}.${format}`,
          new Blob([text], {
            type: format === "otio" ? "application/json" : "text/plain",
          }),
        );
        if (left.length) onStatus(left.join("  ·  "));
      } catch (e) {
        onStatus(m["share.failed"]({ reason: (e as Error).message }));
      } finally {
        setBusy(false);
      }
    },
    [folderFor, place, onStatus],
  );

  const reveal = useCallback(async () => {
    if (last) await host.reveal(last).catch(() => {});
  }, [host, last]);

  const shareLast = useCallback(async () => {
    if (last) await host.share(last).catch(() => {});
  }, [host, last]);

  return {
    busy,
    last,
    host,
    reveal,
    shareLast,
    exportMix,
    exportTracks,
    exportVideo,
    exportTimeline,
  };
}
