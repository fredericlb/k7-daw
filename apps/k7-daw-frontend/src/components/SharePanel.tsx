// SharePanel.tsx — the Export panel: the person picks WHAT leaves (an MP3/WAV, each track's audio in a zip, a
// video), then "Choose a folder…" opens the system picker and the export runs; the result names the folder.
// A PANEL docked beside the panes, an export taking a minute and saying where the file went. See ../docs/export.md.
// The chosen entry is a COMMAND LINE the shell dispatches (`project export`), not a call of its own: that keeps
// this and the palette saying the same. The running card reads the page's process list (../processes.ts).

import { Button } from "@studio/ui/button";
import { Chip } from "@studio/ui/chip";
import { Download, FileArchive, FolderOpen } from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import { Progress } from "@studio/ui/progress";
import { RowButton, RowGroup } from "@studio/ui/row-group";
import { useEffect, useState, useSyncExternalStore } from "react";
import { tracksLine } from "../exportArgs";
import type { ExportHost, Saved } from "../exportHost";
import { m } from "../paraglide/messages.js";
import { elapsedOf, processesSnapshot, subscribeProcesses } from "../processes";
import { VideoExportRow } from "./VideoExportRow";

/** The running card: what renders, how far when the render says, and how long it has taken. */
function RunningCard({ timeline }: { timeline: string }) {
  const all = useSyncExternalStore(
    subscribeProcesses,
    processesSnapshot,
    processesSnapshot,
  );
  // The newest running export this page tracks; the engine's own mixdown reports no fraction.
  const job = [...all]
    .reverse()
    .find(
      (p) => p.local && p.state === "running" && p.label.startsWith("export"),
    );
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);
  const pct =
    job?.progress === undefined ? null : Math.floor(job.progress * 100);
  const what = job ? job.label.replace(/^export /, "") : "";
  return (
    <div
      role="status"
      className="mb-2 shrink-0 rounded-2xl bg-(--k7-row) px-3.5 py-3"
    >
      <div className="flex items-baseline gap-3">
        <span className="type-body min-w-0 flex-1 truncate">
          {what ? m["share.renderingWhat"]({ what }) : m["share.rendering2"]()}
        </span>
        {pct === null ? null : (
          <span className="type-mono-value text-fg-2">{pct} %</span>
        )}
      </div>
      <Progress
        value={pct ?? 100}
        label={what || m["share.rendering2"]()}
        className={cn("my-2", pct === null && "animate-pulse")}
      />
      <p className="type-status text-fg-3">
        {[
          timeline || m["tl.main"](),
          job ? m["share.elapsed"]({ time: elapsedOf(job, now) }) : "",
        ]
          .filter(Boolean)
          .join(" · ")}
      </p>
    </div>
  );
}

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
      {busy ? <RunningCard timeline={chosen} /> : null}

      <div className="min-h-0 flex-1 overflow-y-auto">
        <h3 className="type-caps pb-2 text-fg-2">{m["share.mixdown"]()}</h3>
        {/* Only once there is a choice, as the timeline pane's tabs are. */}
        {timelines.length ? (
          <div
            role="radiogroup"
            aria-label={m["share.timeline"]()}
            className="mb-2 flex flex-wrap gap-2"
          >
            {["", ...timelines].map((t) => (
              <Chip
                key={t || "main"}
                variant="filter"
                role="radio"
                aria-checked={chosen === t}
                selected={chosen === t}
                onClick={() => {
                  setTimeline(t);
                  setLine("");
                }}
              >
                {t || m["tl.main"]()}
              </Chip>
            ))}
          </div>
        ) : null}
        <RowGroup>
          {(["mp3", "wav"] as const).map((f) => (
            <RowButton
              key={f}
              tall
              icon={<Download />}
              detail={
                f === "wav"
                  ? m["share.detailWav"]()
                  : m["share.detailMix"]({ timeline: chosen || m["tl.main"]() })
              }
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
            detail={m["share.detailTracks"]()}
            selected={pressed(tracksLine(chosen || undefined))}
            disabled={busy}
            onClick={() => choose(tracksLine(chosen || undefined))}
          >
            {m["share.exportTracks"]()}
          </RowButton>
        </RowGroup>
        {/* …and the PICTURE, on a project that draws one. */}
        {hasPicture ? (
          <>
            <h3 className="type-caps pt-4 pb-2 text-fg-2">
              {m["share.video"]()}
            </h3>
            <RowGroup>
              <VideoExportRow busy={busy} chosen={line} onChoose={choose} />
            </RowGroup>
          </>
        ) : null}
        <p className="type-status px-1 pt-3 text-fg-3">{m["share.note"]()}</p>
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
