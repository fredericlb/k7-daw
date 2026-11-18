// SharePanel.tsx — render the project through the engine (an MP3/WAV download, each track's
// audio in a zip, a video). A PANEL anchored at the right, docked beside
// the panes, a dropdown being the wrong shape for a surface that reports — an export takes a
// minute and says where the file went. See ../docs/shell.md.
// Every entry is a COMMAND LINE the shell dispatches (`project export`),
// not a call of its own: that keeps this and the palette saying the same.

import { Button } from "@studio/ui/button";
import { Download, FileArchive, Loader2, Share2, X } from "@studio/ui/icons";
import { useState } from "react";
import { tracksLine } from "../exportArgs";
import { m } from "../paraglide/messages.js";
import { VideoExportRow } from "./VideoExportRow";

export function SharePanel({
  busy,
  hasPicture,
  timelines = [],
  onRun,
  onClose,
}: {
  /** A render is already running — the rows grey out rather than starting a second. */
  busy: boolean;
  /** Does this project draw a picture? The video row is absent when it does not — a project with
   *  no video document has nothing to render, and a greyed row would not say why. */
  hasPicture?: boolean;
  /** The project's other timelines: with any, the mixdown asks which one — Main, the song, by default. */
  timelines?: readonly string[];
  onRun: (command: string) => void;
  onClose: () => void;
}) {
  const row =
    "flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm hover:bg-accent disabled:opacity-50";
  const head = "px-2 pb-1 pt-2 text-xs font-medium text-muted-foreground";
  // Which timeline the mixdown renders; one that has gone since is Main again.
  const [timeline, setTimeline] = useState("");
  const chosen = timelines.includes(timeline) ? timeline : "";
  const audioLine = (format: string) =>
    `project export audio ${format}${chosen ? ` ${chosen}` : ""}`;

  return (
    <aside className="flex w-[24rem] min-w-0 shrink-0 flex-col border-l border-border bg-background">
      <header className="flex shrink-0 items-center gap-2 border-b border-border px-2 py-1.5">
        <Share2 className="h-4 w-4 shrink-0 text-muted-foreground" />
        <span className="min-w-0 flex-1 truncate text-sm font-medium">
          {m["share.button"]()}
        </span>
        {busy ? (
          <Loader2
            className="h-4 w-4 shrink-0 animate-spin text-muted-foreground"
            aria-label={m["share.rendering2"]()}
          />
        ) : null}
        <Button
          size="icon"
          variant="ghost"
          title={m["common.close"]()}
          onClick={onClose}
        >
          <X className="h-4 w-4" />
        </Button>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        <div className={head}>{m["share.mixdown"]()}</div>
        {/* Only once there is a choice, as the timeline pane's tabs are. */}
        {timelines.length ? (
          <label className="flex items-center gap-2 px-2 py-1 text-sm">
            <span className="text-muted-foreground">
              {m["share.timeline"]()}
            </span>
            <select
              className="min-w-0 flex-1 rounded border border-border bg-background px-1 py-0.5"
              value={chosen}
              disabled={busy}
              onChange={(e) => setTimeline(e.target.value)}
            >
              <option value="">{m["tl.main"]()}</option>
              {timelines.map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </select>
          </label>
        ) : null}
        <button
          type="button"
          className={row}
          disabled={busy}
          onClick={() => onRun(audioLine("mp3"))}
        >
          <Download className="h-4 w-4 text-muted-foreground" />
          {m["share.exportAudio"]({ format: "MP3" })}
        </button>
        <button
          type="button"
          className={row}
          disabled={busy}
          onClick={() => onRun(audioLine("wav"))}
        >
          <Download className="h-4 w-4 text-muted-foreground" />
          {m["share.exportAudio"]({ format: "WAV" })}
        </button>

        <button
          type="button"
          className={row}
          disabled={busy}
          onClick={() => onRun(tracksLine(chosen || undefined))}
        >
          <FileArchive className="h-4 w-4 text-muted-foreground" />
          {m["share.exportTracks"]()}
        </button>

        {/* …and the PICTURE, on a project that draws one. The sound is married in: a video of a
            song is a video of its sound, so the silent one is `… video <container> none`. */}
        {hasPicture ? (
          <VideoExportRow busy={busy} rowClass={row} onRun={onRun} />
        ) : null}
      </div>
    </aside>
  );
}
