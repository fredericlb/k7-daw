// RunningBubble.tsx — the spinner at the app's top right while something runs; a press opens a bubble listing what
// runs, each with how far it is and for how long. Nothing runs, nothing is drawn; an ending is a toast (docs/processes.md).

import { Popover, PopoverContent, PopoverTrigger } from "@studio/ui/popover";
import { Progress } from "@studio/ui/progress";
import { Row, RowGroup } from "@studio/ui/row-group";
import { Spinner } from "@studio/ui/spinner";
import { useEffect, useState, useSyncExternalStore } from "react";
import { m } from "../paraglide/messages.js";
import {
  elapsedOf,
  processesSnapshot,
  runningCount,
  type ShownProcess,
  subscribeProcesses,
} from "../processes";

/** One running process: its line, how far when it says, and how long; the list holds nothing else. */
function ProcessRow({ p, now }: { p: ShownProcess; now: number }) {
  const pct = p.progress !== undefined ? Math.floor(p.progress * 100) : null;
  const said = [
    pct === null ? m["processes.running"]() : `${pct} %`,
    p.detail,
    elapsedOf(p, now),
    p.local ? m["processes.here"]() : undefined,
  ]
    .filter(Boolean)
    .join(" · ");
  return (
    <div>
      <Row detail={said} trailing={<Spinner />}>
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
function useProcessClock(all: readonly ShownProcess[]): number {
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

export function RunningBubble({ className }: { className?: string }) {
  const all = useSyncExternalStore(
    subscribeProcesses,
    processesSnapshot,
    processesSnapshot,
  );
  const now = useProcessClock(all);
  const live = all.filter((p) => p.state === "running");
  if (!live.length) return null;
  const label = m["shell.processes"]({ n: live.length });
  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label={label}
          title={label}
          className={`float grid size-(--k7-float-h) shrink-0 cursor-pointer place-items-center rounded-full outline-none focus-visible:shadow-(--k7-focus-ring) ${className ?? ""}`}
        >
          <Spinner />
        </button>
      </PopoverTrigger>
      <PopoverContent
        side="bottom"
        align="end"
        aria-label={m["processes.running"]()}
        className="flex max-h-[60vh] w-80 flex-col overflow-y-auto p-2"
      >
        <RowGroup>
          {live.map((p) => (
            <ProcessRow key={p.id} p={p} now={now} />
          ))}
        </RowGroup>
      </PopoverContent>
    </Popover>
  );
}
