// LogsPanel.tsx — the Logs panel: a transcript of engine and brain activity, newest line last.
// The app shell keeps the buffer; the tab (All, Problems, Commands, Engine) is the store's `logsFilter`, which the header chip sets.

import { Button } from "@studio/ui/button";
import { Chip } from "@studio/ui/chip";
import { EmptyState } from "@studio/ui/empty-state";
import {
  Ban,
  Cpu,
  FileText,
  Info,
  SquareTerminal,
  TriangleAlert,
} from "@studio/ui/icons";
import { Input } from "@studio/ui/input";
import { cn } from "@studio/ui/lib/utils";
import { Row, RowGroup } from "@studio/ui/row-group";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { m } from "../paraglide/messages.js";
import { useShell } from "../state/shellStore";
import type { LogsFilter } from "../state/toggleSlice";

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

const TABS: readonly LogsFilter[] = ["all", "problems", "commands", "engine"];

/** Which tab a line is listed under; a line may sit in `all` and one more. */
const IN_TAB: Record<LogsFilter, (l: LogLine) => boolean> = {
  all: () => true,
  problems: isProblem,
  commands: (l) => l.kind === "cmd",
  engine: (l) => l.kind === "engine" || l.kind === "file",
};

const TAB_LABEL: Record<LogsFilter, (n: number) => string> = {
  all: (n) => m["logs.all"]({ n }),
  problems: (n) => m["logs.problems"]({ n }),
  commands: (n) => m["logs.commands"]({ n }),
  engine: (n) => m["logs.engine"]({ n }),
};

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
  const [text, setText] = useState("");
  const needle = text.trim().toLowerCase();
  const shown = lines.filter(
    (l) =>
      IN_TAB[filter](l) && (!needle || l.text.toLowerCase().includes(needle)),
  );
  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [shown]);
  const bare = lines.length === 0;
  const emptyTitle = bare ? m["logs.emptyTitle"]() : m["logs.noneTitle"]();
  const emptyText = bare ? m["logs.emptyText"]() : m["logs.noneText"]();

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 flex-wrap items-center gap-2 pb-2">
        {TABS.map((t) => (
          <Chip
            key={t}
            variant="filter"
            selected={filter === t}
            onClick={() => setFilter(t)}
          >
            {TAB_LABEL[t](lines.filter(IN_TAB[t]).length)}
          </Chip>
        ))}
      </div>
      <Input
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder={m["logs.filter"]()}
        aria-label={m["logs.filter"]()}
        className="mb-2 shrink-0"
      />
      <div className="min-h-0 flex-1 overflow-auto">
        {shown.length === 0 ? (
          <EmptyState compact glyph={<Info />} title={emptyTitle}>
            {emptyText}
          </EmptyState>
        ) : (          <RowGroup>
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
      <div className="flex shrink-0 items-center gap-2 pt-2">
        <span className="type-status mr-auto text-fg-3">
          {m["logs.count"]({ n: shown.length })}
        </span>
        <Button
          size="sm"
          variant="ghost"
          disabled={shown.length === 0}
          onClick={() =>
            void navigator.clipboard?.writeText(
              shown.map((l) => l.text).join("\n"),
            )
          }
        >
          {m["logs.copy"]()}
        </Button>
        <Button size="sm" variant="ghost" onClick={onClear}>
          {m["logs.clear"]()}
        </Button>
      </div>
    </div>
  );
}
