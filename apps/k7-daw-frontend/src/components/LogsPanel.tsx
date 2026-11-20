// LogsPanel.tsx — the Logs panel: a transcript of engine and brain activity, newest line last.
// The app shell keeps the buffer; the All | Problems filter is the store's `logsFilter`, which the header chip sets.

import { Button } from "@studio/ui/button";
import { Chip } from "@studio/ui/chip";
import { useEffect, useRef } from "react";
import { m } from "../paraglide/messages.js";
import { useShell } from "../state/shellStore";

export interface LogLine {
  t: number; // epoch ms
  // cmd/info/refused/error/engine are in-app activity; `engine` and `file` also carry the engine machine's merged process log.
  kind: "cmd" | "engine" | "file" | "info" | "refused" | "error";
  text: string;
}

const KIND_CLASS: Record<LogLine["kind"], string> = {
  cmd: "text-sky-400",
  engine: "text-violet-400",
  file: "text-teal-400",
  info: "text-muted-foreground",
  // A refusal is not an error but a verb declining, so it is amber and deliberately distinct from `info`.
  refused: "text-amber-400",
  error: "text-red-400",
};

/** A problem is a line that failed or that a verb refused. */
const isProblem = (l: LogLine): boolean =>
  l.kind === "error" || l.kind === "refused";

export function LogsPanel({
  lines,
  onClear,
}: {
  lines: LogLine[];
  onClear: () => void;
}) {
  const endRef = useRef<HTMLDivElement>(null);
  const filter = useShell((st) => st.logsFilter);
  const setFilter = useShell((st) => st.setLogsFilter);
  const problems = lines.filter(isProblem);
  const shown = filter === "problems" ? problems : lines;
  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [shown]);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 items-center gap-2 pb-2">
        <Chip
          variant="filter"
          selected={filter === "all"}
          onClick={() => setFilter("all")}
        >
          {m["logs.all"]()}
        </Chip>
        <Chip
          variant="filter"
          selected={filter === "problems"}
          onClick={() => setFilter("problems")}
        >
          {m["logs.problems"]({ n: problems.length })}
        </Chip>
        <Button size="sm" variant="ghost" className="ml-auto" onClick={onClear}>
          {m["logs.clear"]()}
        </Button>
      </div>
      <div className="min-h-0 flex-1 overflow-auto px-2 py-1 font-mono text-[11px] leading-relaxed">
        {shown.length === 0 ? (
          <div className="text-muted-foreground">
            {filter === "problems" ? m["logs.noProblems"]() : m["logs.empty"]()}
          </div>
        ) : (
          shown.map((l, i) => (
            <div key={i} className="flex gap-2">
              <span className="shrink-0 text-faint">
                {new Date(l.t).toLocaleTimeString()}
              </span>
              <span
                className={`whitespace-pre-wrap break-all ${KIND_CLASS[l.kind]}`}
              >
                {l.text}
              </span>
            </div>
          ))
        )}
        <div ref={endRef} />
      </div>
    </div>
  );
}
