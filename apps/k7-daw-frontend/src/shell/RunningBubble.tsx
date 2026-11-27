// RunningBubble.tsx — the spinner at the app's top right while something runs; a press opens a bubble listing what
// runs, each with how far it is and for how long (ProcessesSection.tsx's rows). Nothing runs, nothing is drawn.

import { Popover, PopoverContent, PopoverTrigger } from "@studio/ui/popover";
import { RowGroup } from "@studio/ui/row-group";
import { Spinner } from "@studio/ui/spinner";
import { useSyncExternalStore } from "react";
import { ProcessRow, useProcessClock } from "../components/ProcessesSection";
import { m } from "../paraglide/messages.js";
import { processesSnapshot, subscribeProcesses } from "../processes";

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
