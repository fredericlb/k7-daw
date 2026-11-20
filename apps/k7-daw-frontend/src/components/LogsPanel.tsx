// LogsPanel.tsx — the Logs panel: a transcript of engine and brain activity, newest line last.
// The app shell keeps the buffer; the All | Problems filter is the store's `logsFilter`, which the header chip sets.

import { Button } from "@studio/ui/button";
import { Chip } from "@studio/ui/chip";
import {
  Ban,
  Cpu,
  FileText,
  Info,
  SquareTerminal,
  TriangleAlert,
} from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import { Row, RowGroup } from "@studio/ui/row-group";
import { type ReactNode, useEffect, useRef } from "react";
import { m } from "../paraglide/messages.js";
import { useShell } from "../state/shellStore";

export interface LogLine {
  t: number; // epoch ms
  // cmd/info/refused/error/engine are in-app activity; `engine` and `file` also carry the engine machine's merged process log.
  kind: "cmd" | "engine" | "file" | "info" | "refused" | "error";
  text: string;
}

const KIND_CLASS: Record<LogLine["kind"], string> = {
  cmd: "text-foreground",
  engine: "text-fg-2",
  file: "text-fg-2",
  info: "text-fg-2",
  // A refusal is not an error but a verb declining, so it wears the warn colour, distinct from an error.
  refused: "text-warn",
  error: "text-error",
};

const KIND_ICON: Record<LogLine["kind"], ReactNode> = {
  cmd: <SquareTerminal />,
  engine: <Cpu />,
  file: <FileText />,
  info: <Info />,
  refused: <Ban />,
  error: <TriangleAlert />,
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
      <div className="min-h-0 flex-1 overflow-auto">
        {shown.length === 0 ? (
          <p className="px-1 py-2 text-fg-2 text-sm">
            {filter === "problems" ? m["logs.noProblems"]() : m["logs.empty"]()}
          </p>
        ) : (
          <RowGroup>
            {shown.map((l, i) => (
              <Row
                // biome-ignore lint/suspicious/noArrayIndexKey: a log line has no id; the list only grows at its end.
                key={i}
                icon={KIND_ICON[l.kind]}
                className={cn(
                  "items-start",
                  isProblem(l) && "bg-(--k7-warn-fill)",
                )}
                detail={new Date(l.t).toLocaleTimeString()}
              >
                <span
                  className={cn(
                    "type-mono-value whitespace-pre-wrap break-all",
                    KIND_CLASS[l.kind],
                  )}
                >
                  {l.text}
                </span>
              </Row>
            ))}
          </RowGroup>
        )}
        <div ref={endRef} />
      </div>
    </div>
  );
}
