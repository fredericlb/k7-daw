// SharePanel.tsx — the Export panel: git's Save and Update first, then the person picks WHAT leaves (an MP3/WAV, each track's audio in a zip, a
// video); pressing a row opens the system's folder picker at once and the export runs; the result names the folder.
// A PANEL docked beside the panes, an export taking a minute and saying where the file went. See ../docs/export.md.
// The chosen entry is a COMMAND LINE the shell dispatches (`project export`), not a call of its own: that keeps
// this and the palette saying the same. The running card reads the page's process list (../processes.ts).

import { tracksLine } from "@studio/k7-wasm/edit";
import { Button } from "@studio/ui/button";
import { Chip } from "@studio/ui/chip";
import { Download, FileArchive } from "@studio/ui/icons";
import { Progress } from "@studio/ui/progress";
import { RowButton, RowGroup } from "@studio/ui/row-group";
import { SectionHeading } from "@studio/ui/section-heading";
import {
  type ReactNode,
  useEffect,
  useState,
  useSyncExternalStore,
} from "react";
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
        value={pct ?? undefined}
        label={what || m["share.rendering2"]()}
        className="my-2"
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
  sync,
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
  /** Git's Save and Update, first in the panel; absent outside a git workspace (HeaderGit.tsx's GitShareRows). */
  sync?: ReactNode;
}) {
  // Which timeline the mixdown renders; one that has gone since is Main again.
  const [timeline, setTimeline] = useState("");
  const chosen = timelines.includes(timeline) ? timeline : "";
  const audioLine = (format: string) =>
    `project export audio ${format}${chosen ? ` ${chosen}` : ""}`;
  // The line whose folder picker is open; pressing a row runs it at once, and the card below offers a retry or a way out.
  const [line, setLine] = useState("");
  const choose = (l: string) => {
    setLine(l);
    onRun(l);
  };
  // A render started, or a file landed: the picker has been answered.
  useEffect(() => {
    if (busy || last) setLine("");
  }, [busy, last]);

  const pressed = (l: string) => line === l;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {busy ? <RunningCard timeline={chosen} /> : null}

      <div className="min-h-0 flex-1 overflow-y-auto">
        {sync ? (
          <>
            <SectionHeading className="pb-2">
              {m["share.sync"]()}
            </SectionHeading>
            {sync}
            <div className="pt-4" />
          </>
        ) : null}
        <SectionHeading className="pb-2">{m["share.mixdown"]()}</SectionHeading>
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
            <SectionHeading className="pt-4 pb-2">
              {m["share.video"]()}
            </SectionHeading>
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
            <p className="type-mono-value truncate text-fg-1">
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
        {line && host?.canPick && !busy ? (
          <section
            aria-label={m["share.whereTitle"]()}
            className="rounded-2xl bg-(--k7-row) p-3.5"
          >
            <p className="type-body">{m["share.whereTitle"]()}</p>
            <p className="type-status pt-1 text-fg-3">
              {m["share.whereOpen"]()}
            </p>
            <div className="flex gap-2 pt-2.5">
              <Button size="sm" onClick={() => onRun(line)}>
                {m["share.choose"]()}
              </Button>
              <Button size="sm" variant="secondary" onClick={() => setLine("")}>
                {m["share.cancel"]()}
              </Button>
            </div>
          </section>
        ) : null}
      </footer>
    </div>
  );
}
