// ProcessesSection.tsx — the ENDED background processes of Tasks & history, a kit row each with its outcome and its failure,
// and the row the spinner's bubble lists what runs with. The list is ../processes.ts (docs/processes.md).

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

export function ProcessRow({ p, now }: { p: ShownProcess; now: number }) {
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

/** The clock the rows' elapsed times read: a second's tick while something runs. */
export function useProcessClock(all: readonly ShownProcess[]): number {
  const running = runningCount(all);
  const [now, setNow] = useState(() => Date.now());
  // biome-ignore lint/correctness/useExhaustiveDependencies: `all` restarts the clock when the list changes.
  useEffect(() => {
    setNow(Date.now());
    if (!running) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [running, all]);
  return now;
}

/** The ended processes of Tasks & history, each with its outcome; what still runs is the spinner's bubble (RunningBubble.tsx). */
export function ProcessesSection() {
  const all = useSyncExternalStore(
    subscribeProcesses,
    processesSnapshot,
    processesSnapshot,
  );
  const now = useProcessClock(all);
  const ended = all.filter((p) => p.state !== "running");
  if (!ended.length) return null;
  return (
    <section aria-label={m["processes.heading"]()} className="shrink-0 pb-3">
      <div className="flex items-center pb-2">
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
      <RowGroup>
        {ended.map((p) => (
          <ProcessRow key={p.id} p={p} now={now} />
        ))}
      </RowGroup>
    </section>
  );
}
