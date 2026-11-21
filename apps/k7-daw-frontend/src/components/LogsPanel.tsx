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
import { isCommandLine, isProblemLine } from "../logProblems";
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

const TABS: readonly LogsFilter[] = ["all", "problems", "commands", "engine"];

/** Which tab a line is listed under; a line may sit in `all` and one more. */
const IN_TAB: Record<LogsFilter, (l: LogLine) => boolean> = {
  all: () => true,
  problems: isProblemLine,
  commands: isCommandLine,
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
  warnings = [],
}: {
  lines: LogLine[];
  onClear: () => void;
  /** The in-place warnings (a module or lane that complains), listed first under Problems. */
  warnings?: readonly string[];
}) {
  const endRef = useRef<HTMLDivElement>(null);
  /** Whether the list is scrolled to its end: only then does a new line pull the view down. */
  const following = useRef(true);
  const [copied, setCopied] = useState(false);
  const filter = useShell((st) => st.logsFilter);
  const setFilter = useShell((st) => st.setLogsFilter);
  const [text, setText] = useState("");
  const needle = text.trim().toLowerCase();
  const matches = (t: string) => !needle || t.toLowerCase().includes(needle);
  const shown = lines.filter((l) => IN_TAB[filter](l) && matches(l.text));
  const shownWarnings = filter === "problems" ? warnings.filter(matches) : [];
  const count = shown.length + shownWarnings.length;
  // biome-ignore lint/correctness/useExhaustiveDependencies: `count` and `filter` are the triggers — the list's own arrays are new on every render.
  useEffect(() => {
    if (following.current) endRef.current?.scrollIntoView({ block: "end" });
  }, [count, filter]);
  const copy = async () => {
    const all = [...shownWarnings, ...shown.map((l) => l.text)].join("\n");
    try {
      await navigator.clipboard?.writeText(all);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      setCopied(false);
    }
  };
  const bare = lines.length === 0 && warnings.length === 0;
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
            {TAB_LABEL[t](
              lines.filter(IN_TAB[t]).length +
                (t === "problems" ? warnings.length : 0),
            )}
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
      <div
        className="min-h-0 flex-1 overflow-auto"
        onScroll={(e) => {
          const el = e.currentTarget;
          following.current =
            el.scrollHeight - el.scrollTop - el.clientHeight < 24;
        }}
      >
        {shown.length + shownWarnings.length === 0 ? (
          <EmptyState compact glyph={<Info />} title={emptyTitle}>
            {emptyText}
          </EmptyState>
        ) : (
          <RowGroup>
            {shownWarnings.map((w) => (
              <Row
                key={w}
                icon={<TriangleAlert />}
                className="items-start bg-(--k7-warn-fill)"
              >
                <span className="type-mono-value whitespace-pre-wrap break-all text-warn">
                  {w}
                </span>
              </Row>
            ))}
            {shown.map((l, i) => (
              <Row
                // biome-ignore lint/suspicious/noArrayIndexKey: a log line has no id; the list only grows at its end.
                key={i}
                icon={KIND_ICON[l.kind]}
                className={cn(
                  "items-start",
                  isProblemLine(l) && "bg-(--k7-warn-fill)",
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
          {m["logs.count"]({ n: shown.length + shownWarnings.length })}
        </span>
        <Button
          size="sm"
          variant="ghost"
          disabled={count === 0}
          onClick={() => void copy()}
        >
          {copied ? m["logs.copied"]() : m["logs.copy"]()}
        </Button>
        <Button size="sm" variant="ghost" onClick={onClear}>
          {m["logs.clear"]()}
        </Button>
      </div>
    </div>
  );
}
