// ProcessesSection.tsx — the background processes of Tasks & history: a kit row each, with its state and its failure.
// Nothing renders while the list is empty; the list is ../processes.ts (docs/processes.md).

import { Check, CircleAlert } from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import { Progress } from "@studio/ui/progress";
import { Row, RowGroup } from "@studio/ui/row-group";
import { Spinner } from "@studio/ui/spinner";
import { useEffect, useState, useSyncExternalStore } from "react";
import { m } from "../paraglide/messages.js";
import {
  processesSnapshot,
  runningCount,
  type ShownProcess,
  subscribeProcesses,
} from "../processes";

const elapsed = (p: ShownProcess, now: number): string => {
  const s = Math.max(0, Math.round(((p.endedAt ?? now) - p.startedAt) / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};

function ProcessRow({ p, now }: { p: ShownProcess; now: number }) {
  const pct =
    p.state === "running" && p.progress !== undefined
      ? Math.floor(p.progress * 100)
      : null;
  const state = m[`processes.${p.state}`]();
  const said = [
    pct === null ? state : `${pct} %`,
    p.detail,
    elapsed(p, now),
    p.local ? m["processes.here"]() : undefined,
  ]
    .filter(Boolean)
    .join(" · ");
  return (
    <div>
      <Row
        detail={
          <span className={cn(p.state === "failed" && "text-error")}>
            {said}
          </span>
        }
        trailing={
          p.state === "running" ? (
            <Spinner />
          ) : p.state === "done" ? (
            <Check className="size-4 text-ok" aria-label={state} />
          ) : (
            <CircleAlert className="size-4 text-error" aria-label={state} />
          )
        }
      >
        <span className="type-mono-value" title={p.label}>
          {p.label}
        </span>
      </Row>
      {pct !== null ? (
        <Progress
          value={pct}
          label={p.label}
          className="mx-3.5 mb-2.5 w-auto"
        />
      ) : null}
    </div>
  );
}

export function ProcessesSection() {
  const all = useSyncExternalStore(subscribeProcesses, processesSnapshot);
  const running = runningCount(all);
  // A second's tick while something runs, so the elapsed times move.
  const [now, setNow] = useState(() => Date.now());
  // biome-ignore lint/correctness/useExhaustiveDependencies: `all` restarts the clock when the list changes.
  useEffect(() => {
    setNow(Date.now());
    if (!running) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [running, all]);
  if (all.length === 0) return null;
  return (
    <section aria-label={m["processes.heading"]()} className="shrink-0 pb-3">
      <h3 className="type-caps pb-2 text-fg-2">{m["processes.heading"]()}</h3>
      <RowGroup>
        {all.map((p) => (
          <ProcessRow key={p.id} p={p} now={now} />
        ))}
      </RowGroup>
    </section>
  );
}
