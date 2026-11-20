// SharePanel.tsx — the Export panel: the person picks WHAT leaves (an MP3/WAV, each track's audio in a zip, a
// video), then "Choose a folder…" opens the system picker and the export runs; the result names the folder.
// A PANEL docked beside the panes, an export taking a minute and saying where the file went. See ../docs/export.md.
// The chosen entry is a COMMAND LINE the shell dispatches (`project export`), not a call of its own: that keeps
// this and the palette saying the same.

import { Button } from "@studio/ui/button";
import {
  Download,
  FileArchive,
  FolderOpen,
  Loader2,
  Share2,
  X,
} from "@studio/ui/icons";
import { useState } from "react";
import { tracksLine } from "../exportArgs";
import type { ExportHost, Saved } from "../exportHost";
import { m } from "../paraglide/messages.js";
import { VideoExportRow } from "./VideoExportRow";

export function SharePanel({
  busy,
  hasPicture,
  timelines = [],
  host,
  last = null,
  onReveal,
  onShare,
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
  /** Where exports go; absent, a plain browser's download. */
  host?: Pick<ExportHost, "canPick" | "canReveal" | "canShare">;
  /** The file the last export saved, named under the action. */
  last?: Saved | null;
  onReveal?: () => void;
  onShare?: () => void;
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
  // What leaves, as the command line that does it; nothing runs until the folder step.
  const [line, setLine] = useState("");
  const choose = (l: string) => setLine(l);
  const on = (l: string) => (line === l ? "bg-accent" : "");

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
          className={`${row} ${on(audioLine("mp3"))}`}
          aria-pressed={line === audioLine("mp3")}
          disabled={busy}
          onClick={() => choose(audioLine("mp3"))}
        >
          <Download className="h-4 w-4 text-muted-foreground" />
          {m["share.exportAudio"]({ format: "MP3" })}
        </button>
        <button
          type="button"
          className={`${row} ${on(audioLine("wav"))}`}
          aria-pressed={line === audioLine("wav")}
          disabled={busy}
          onClick={() => choose(audioLine("wav"))}
        >
          <Download className="h-4 w-4 text-muted-foreground" />
          {m["share.exportAudio"]({ format: "WAV" })}
        </button>

        <button
          type="button"
          className={`${row} ${on(tracksLine(chosen || undefined))}`}
          aria-pressed={line === tracksLine(chosen || undefined)}
          disabled={busy}
          onClick={() => choose(tracksLine(chosen || undefined))}
        >
          <FileArchive className="h-4 w-4 text-muted-foreground" />
          {m["share.exportTracks"]()}
        </button>

        {/* …and the PICTURE, on a project that draws one. The sound is married in: a video of a
            song is a video of its sound, so the silent one is `… video <container> none`. */}
        {hasPicture ? (
          <VideoExportRow
            busy={busy}
            rowClass={row}
            chosen={line}
            onChoose={choose}
          />
        ) : null}
      </div>

      <footer className="shrink-0 border-t border-border p-2">
        <Button
          className="w-full"
          disabled={busy || !line}
          onClick={() => onRun(line)}
        >
          {host?.canPick ? (
            <FolderOpen className="mr-2 h-4 w-4" />
          ) : (
            <Download className="mr-2 h-4 w-4" />
          )}
          {host?.canPick ? m["share.choose"]() : m["share.download"]()}
        </Button>
        {last && host?.canPick ? (
          <div className="mt-2 text-sm">
            <p className="truncate text-muted-foreground" title={last.where}>
              {m["share.savedIn"]({ folder: last.folder.name })} · {last.name}
            </p>
            <div className="mt-1 flex gap-2">
              {host.canReveal ? (
                <Button size="sm" variant="outline" onClick={onReveal}>
                  {m["share.showInFolder"]()}
                </Button>
              ) : null}
              {host.canShare ? (
                <Button size="sm" variant="outline" onClick={onShare}>
                  {m["share.shareFile"]()}
                </Button>
              ) : null}
            </div>
          </div>
        ) : null}
      </footer>
    </aside>
  );
}
