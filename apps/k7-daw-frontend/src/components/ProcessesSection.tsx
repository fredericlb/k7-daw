// ProcessesSection.tsx — the background processes of Tasks & history: a kit row each, with its state and its failure.
// An empty list still draws, with the note that says when work shows here; the list is ../processes.ts (docs/processes.md).

import { Button } from "@studio/ui/button";
import { Check, CircleAlert } from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import { Progress } from "@studio/ui/progress";
import { Row, RowGroup } from "@studio/ui/row-group";
import { Spinner } from "@studio/ui/spinner";
import { useEffect, useState, useSyncExternalStore } from "react";
import { m } from "../paraglide/messages.js";
import {
  clearEndedProcesses,
  elapsedOf,
  processesSnapshot,
  runningCount,
  type ShownProcess,
  subscribeProcesses,
} from "../processes";

function ProcessRow({ p, now }: { p: ShownProcess; now: number }) {
  const pct =
    p.state === "running" && p.progress !== undefined
      ? Math.floor(p.progress * 100)
      : null;
  const state = m[`processes.${p.state}`]();
  const said = [
    pct === null ? state : `${pct} %`,
    p.detail,
    elapsedOf(p, now),
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
  const all = useSyncExternalStore(
    subscribeProcesses,
    processesSnapshot,
    processesSnapshot,
  );
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
  const live = all.filter((p) => p.state === "running");
  const ended = all.filter((p) => p.state !== "running");
  const list = (rows: readonly ShownProcess[]) => (
    <RowGroup>
      {rows.map((p) => (
        <ProcessRow key={p.id} p={p} now={now} />
      ))}
    </RowGroup>
  );
  return (
    <section aria-label={m["processes.heading"]()} className="shrink-0 pb-3">
      <h3 className="type-caps pb-2 text-fg-2">
        {m["processes.running"]()} · {live.length}
      </h3>
      {live.length ? (
        list(live)
      ) : (
        <p className="type-status pb-2 text-fg-3">
          {m["processes.nothingRunning"]()}
        </p>
      )}
      {ended.length ? (
        <>
          <div className="flex items-center pt-3 pb-2">
            <h3 className="type-caps text-fg-2">{m["processes.ended"]()}</h3>
            <Button
              size="sm"
              variant="ghost"
              className="ml-auto"
              onClick={clearEndedProcesses}
            >
              {m["processes.clear"]()}
            </Button>
          </div>
          {list(ended)}
        </>
      ) : null}
      <p className="type-status pt-2 text-fg-3">{m["processes.note"]()}</p>
    </section>
  );
}
