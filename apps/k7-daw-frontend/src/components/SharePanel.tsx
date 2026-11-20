// SharePanel.tsx — the Export panel: the person picks WHAT leaves (an MP3/WAV, each track's audio in a zip, a
// video), then "Choose a folder…" opens the system picker and the export runs; the result names the folder.
// A PANEL docked beside the panes, an export taking a minute and saying where the file went. See ../docs/export.md.
// The chosen entry is a COMMAND LINE the shell dispatches (`project export`), not a call of its own: that keeps
// this and the palette saying the same.

import { Button } from "@studio/ui/button";
import { Download, FileArchive, FolderOpen } from "@studio/ui/icons";
import { Picker } from "@studio/ui/picker";
import { RowButton, RowGroup } from "@studio/ui/row-group";
import { Spinner } from "@studio/ui/spinner";
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
}) {
  // Which timeline the mixdown renders; one that has gone since is Main again.
  const [timeline, setTimeline] = useState("");
  const chosen = timelines.includes(timeline) ? timeline : "";
  const audioLine = (format: string) =>
    `project export audio ${format}${chosen ? ` ${chosen}` : ""}`;
  // What leaves, as the command line that does it; nothing runs until the folder step.
  const [line, setLine] = useState("");
  const choose = (l: string) => setLine(l);

  const pressed = (l: string) => line === l;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {busy ? (
        <div
          role="status"
          className="mb-2 flex shrink-0 items-center gap-3 rounded-2xl bg-(--k7-row) px-3.5 py-3"
        >
          <Spinner />
          <span className="type-body">{m["share.rendering2"]()}</span>
        </div>
      ) : null}

      <div className="min-h-0 flex-1 overflow-y-auto">
        <h3 className="type-caps pb-2 text-fg-2">{m["share.mixdown"]()}</h3>
        {/* Only once there is a choice, as the timeline pane's tabs are. */}
        {timelines.length ? (
          <Picker
            className="mb-2"
            items={[
              { value: "", label: m["tl.main"]() },
              ...timelines.map((t) => ({ value: t, label: t })),
            ]}
            value={chosen}
            onValueChange={(t) => {
              setTimeline(t);
              setLine("");
            }}
            label={m["share.timeline"]()}
            searchLabel={m["share.timeline"]()}
            emptyLabel={m["share.timeline"]()}
          />
        ) : null}
        <RowGroup>
          {(["mp3", "wav"] as const).map((f) => (
            <RowButton
              key={f}
              tall
              icon={<Download />}
              selected={pressed(audioLine(f))}
              disabled={busy}
              onClick={() => choose(audioLine(f))}
            >
              {m["share.exportAudio"]({ format: f.toUpperCase() })}
            </RowButton>
          ))}
          <RowButton
            tall
            icon={<FileArchive />}
            selected={pressed(tracksLine(chosen || undefined))}
            disabled={busy}
            onClick={() => choose(tracksLine(chosen || undefined))}
          >
            {m["share.exportTracks"]()}
          </RowButton>
          {/* …and the PICTURE, on a project that draws one. */}
          {hasPicture ? (
            <VideoExportRow busy={busy} chosen={line} onChoose={choose} />
          ) : null}
        </RowGroup>
      </div>

      <footer className="shrink-0 pt-3">
        {last && host?.canPick ? (
          <section
            aria-label={m["share.savedIn"]({ folder: last.folder.name })}
            className="mb-3 rounded-2xl border border-(--k7-ok-line) bg-(--k7-ok-fill) p-3.5"
          >
            <p className="type-mono-value truncate text-foreground">
              {last.name}
            </p>
            <p className="type-body pt-1">
              {m["share.savedIn"]({ folder: last.folder.name })}
            </p>
            <p
              className="type-status truncate font-mono text-fg-3"
              title={last.where}
            >
              {last.where}
            </p>
            <div className="flex gap-2 pt-2.5">
              {host.canReveal ? (
                <Button size="sm" variant="secondary" onClick={onReveal}>
                  {m["share.showInFolder"]()}
                </Button>
              ) : null}
              {host.canShare ? (
                <Button size="sm" variant="secondary" onClick={onShare}>
                  {m["share.shareFile"]()}
                </Button>
              ) : null}
            </div>
          </section>
        ) : null}
        <Button
          className="w-full"
          disabled={busy || !line}
          onClick={() => onRun(line)}
        >
          {host?.canPick ? (
            <FolderOpen className="size-5" />
          ) : (
            <Download className="size-5" />
          )}
          {host?.canPick ? m["share.choose"]() : m["share.download"]()}
        </Button>
      </footer>
    </div>
  );
}
