// DownloadsSection.tsx — App settings › Downloads: what is coming down now (songs, models, instruments, uploads),
// read live from @studio/http-client's store. A failed row stays until cleared, since it asks for a decision.

import {
  clearFinished,
  type Download,
  type DownloadKind,
  downloadsSnapshot,
  subscribeDownloads,
} from "@studio/http-client/downloads";
import {
  AudioLines,
  Check,
  CircleAlert,
  Clock,
  CloudUpload,
  Disc3,
  FolderDown,
  type LucideIcon,
} from "@studio/ui/icons";
import { Progress } from "@studio/ui/progress";
import { Row } from "@studio/ui/row-group";
import { Spinner } from "@studio/ui/spinner";
import { useSyncExternalStore } from "react";
import { m } from "../paraglide/messages.js";
import { Action, Section } from "./Section";

const KIND_ICON: Record<DownloadKind, LucideIcon> = {
  song: Disc3,
  engine: AudioLines,
  upload: CloudUpload,
  workspace: FolderDown,
};

const MB = 1e6;

/** The percentage of a running download whose total is known, else null. */
function percent(d: Download): number | null {
  if (d.phase !== "running" || !d.total) return null;
  return Math.min(100, Math.floor(((d.received ?? 0) / d.total) * 100));
}

/** The line under a row's label: its phase, or how far a running one is. */
function said(d: Download): string {
  if (d.phase === "failed") return d.error ?? m["downloads.failed"]();
  if (d.phase === "queued") return m["downloads.queued"]();
  if (d.phase === "done") return m["downloads.done"]();
  if (!d.received) return m["downloads.running"]();
  const got = (d.received / MB).toFixed(0);
  return d.total
    ? `${percent(d)} % · ${got} / ${(d.total / MB).toFixed(0)} MB`
    : `${got} MB`;
}

function PhaseIcon({ phase }: { phase: Download["phase"] }) {
  if (phase === "running") return <Spinner size="sm" className="text-fg-2" />;
  if (phase === "queued")
    return <Clock className="size-4 text-fg-3" aria-hidden />;
  if (phase === "done") return <Check className="size-4 text-ok" aria-hidden />;
  return <CircleAlert className="size-4 text-error" aria-hidden />;
}

function DownloadRow({ d }: { d: Download }) {
  const Kind = KIND_ICON[d.kind];
  const pct = percent(d);
  return (
    <Row
      detail={
        <>
          <span className={d.phase === "failed" ? "text-error" : undefined}>
            {said(d)}
          </span>
          {pct !== null && (
            <Progress value={pct} label={d.label} className="mt-1.5" />
          )}
        </>
      }
      trailing=<PhaseIcon phase={d.phase} />
    >
      {
        <span className="flex items-center gap-2">
          <Kind className="size-4 shrink-0 text-fg-2" aria-hidden />
          <span className="truncate" title={d.label}>
            {d.label}
          </span>
        </span>
      }
    </Row>
  );
}

export function DownloadsSection() {
  const all = useSyncExternalStore(subscribeDownloads, downloadsSnapshot);
  const finished = all.some((d) => d.phase === "done" || d.phase === "failed");
  return (
    <Section title={m["downloads.heading"]()}>
      {all.length === 0 ? (
        <Row>{m["downloads.empty"]()}</Row>
      ) : (
        all.map((d) => <DownloadRow key={d.key} d={d} />)
      )}
      {finished && (
        <Row
          trailing={
            <Action onClick={clearFinished}>{m["downloads.clear"]()}</Action>
          }
        >
          {""}
        </Row>
      )}
    </Section>
  );
}
