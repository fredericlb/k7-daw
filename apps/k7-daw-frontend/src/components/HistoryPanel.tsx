// HistoryPanel.tsx — the History half of Tasks & history, in three segments: Edits (the undo journal's
// steps, newest first), Workspace (the sync history, syncHistory.ts) and Commits (git's, step 6's list).
// A step reads as its command's sentence (stepSentence.ts), the raw line staying its tooltip.
// Hovering a journal row lights what it touched (historyMarks.ts); the journal changes nothing.
// Restoring is the one write here, and it is a sync edit like any other: apps/k7-daw-backend/docs/files/history.md.

import { askConfirm, askText } from "@studio/project-shell/ask";
import { cfg } from "@studio/project-shell/fileApi";
import { Button } from "@studio/ui/button";
import { Choice } from "@studio/ui/choice";
import {
  Bookmark,
  ChevronDown,
  ChevronRight,
  RotateCcw,
  Undo2,
} from "@studio/ui/icons";
import { RowButton, RowGroup } from "@studio/ui/row-group";
import type { HistoryListEntry } from "@studio/web-protocol";
import {
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useState,
} from "react";
import { api } from "../api";
import { CommitsList } from "../git/CommitsList";
import type { GitTransport } from "../git/gitApi";
import type { GitState } from "../git/gitTypes";
import { type HistoryMark, historyMarks } from "../historyMarks";
import { historyOffers, restoredKept } from "../historySource";
import { useQuietWrites, useRevision } from "../hooks/useRevision";
import { m } from "../paraglide/messages.js";
import { PanelError } from "./PanelError";
import { getLocale } from "../paraglide/runtime.js";
import { usePaneGraph } from "../queries/graph";
import { useReloadKey } from "../state/shellStore";
import {
  historyClient,
  type SyncChange,
  type SyncHistory,
} from "../syncHistory";

/** The 6 px dot that stands for an edit. */
const Dot = () => <span className="size-1.5 rounded-full bg-fg-3" />;

const time = (ts: number) =>
  new Date(ts).toLocaleTimeString(getLocale(), {
    hour: "2-digit",
    minute: "2-digit",
  });
const day = (ts: number) =>
  new Date(ts).toLocaleDateString(getLocale(), {
    weekday: "short",
    day: "numeric",
    month: "short",
  });

export function HistoryPanel({
  describe,
  git = null,
  gitTransport = null,
  onMark,
}: {
  /** The workspace's git state: with a repository the panel offers a second list, the commits. */
  git?: GitState | null;
  gitTransport?: GitTransport | null;
  /** A step's label as a person reads it — its command's sentence and subject; the raw line stays the tooltip. */
  describe?: (label: string) => string;
  /** What the hovered row touched, or null when none is — the graph and the timeline light it. */
  onMark: (mark: HistoryMark | null) => void;
}) {
  const [steps, setSteps] = useState<HistoryListEntry[]>([]);
  const [list, setList] = useState<"edits" | "workspace" | "commits">("edits");
  const underGit = !!git && git.mode !== "none";
  const showCommits = underGit && list === "commits";
  // Only to tell which node of a touched chain a step's label names; a graph that will not build lights the chain.
  const { graph } = usePaneGraph(useReloadKey());
  const [error, setError] = useState("");
  /** The row whose marks are lit: a tap or the keyboard's focus lights one as a pointer's hover does. */
  const [lit, setLit] = useState<string | null>(null);
  const light = (id: string, mark: HistoryMark) => {
    setLit(id);
    onMark(mark);
  };
  const unlight = () => {
    setLit(null);
    onMark(null);
  };
  const revision = useRevision();
  // …and this tab's own knob writes, which are journalled rows too but move no published token.
  const quietWrites = useQuietWrites();

  const refresh = useCallback(async () => {
    try {
      setSteps(await api.historyEntries());
      setError("");
    } catch (e) {
      setError((e as Error).message);
    }
  }, []);
  // On open, and whenever the workspace's revision moves — every edit and every undo is a new row or a moved cursor.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `revision` is the trigger, not an input.
  useEffect(() => {
    void refresh();
  }, [refresh, revision, quietWrites]);
  // Nothing stays lit once the panel is gone.
  useEffect(() => () => onMark(null), [onMark]);

  // Newest first, whatever order the journal answers in.
  const rows = useMemo(() => [...steps].sort((a, b) => b.ts - a.ts), [steps]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {error && (
        <PanelError
          what={m["panel.what.history"]()}
          detail={error}
          onRetry={() => void refresh()}
        />
      )}
      <Choice
        label={m["git.lists"]()}
        value={list}
        onValueChange={setList}
        options={[
          { value: "edits", label: m["git.listEdits"]() },
          { value: "workspace", label: m["history.listWorkspace"]() },
          ...(underGit
            ? [{ value: "commits" as const, label: m["git.listCommits"]() }]
            : []),
        ]}
        className="mb-2 shrink-0"
      />
      {showCommits && (
        <CommitsList
          transport={gitTransport}
          workspace={cfg().workspace}
          state={git}
          refreshKey={[
            git?.ahead,
            git?.behind,
            git?.changed,
            git?.merge,
            git?.lastFetch?.at,
          ].join("/")}
        />
      )}
      <div
        hidden={list !== "edits"}
        className="min-h-0 flex-1 overflow-y-auto"
        onMouseLeave={unlight}
        onBlur={(e) => {
          if (!e.currentTarget.contains(e.relatedTarget as Node | null))
            unlight();
        }}
      >
        <p className="type-status px-1 pt-1 text-fg-3">
          {m["history.intro"]()}
        </p>
        {rows.length === 0 && !error && (
          <p className="px-1 py-2 text-fg-2 text-sm">{m["history.empty"]()}</p>
        )}
        {rows.map((r, i) => {
          const newDay = i === 0 || day(rows[i - 1].ts) !== day(r.ts);
          const { paths, label, undone } = r;
          return (
            <div key={r.id}>
              {newDay && (
                <h3 className="type-caps px-1 pt-3 pb-2 text-fg-2">
                  {day(r.ts)}
                </h3>
              )}
              <RowGroup>
                <RowButton
                  title={`${label}\n${paths.join("\n")}`}
                  className={undone ? "opacity-50" : undefined}
                  icon={undone ? <Undo2 /> : <Dot />}
                  selected={lit === r.id}
                  trailing={
                    <span className="type-mono-value text-fg-3">
                      {time(r.ts)}
                    </span>
                  }
                  onMouseEnter={() =>
                    light(r.id, historyMarks(paths, label, graph))
                  }
                  onFocus={() => light(r.id, historyMarks(paths, label, graph))}
                  onClick={() => light(r.id, historyMarks(paths, label, graph))}
                >
                  {describe ? describe(label) : label}
                </RowButton>
              </RowGroup>
            </div>
          );
        })}
      </div>
      {list === "workspace" && (
        <SyncHistorySection revision={revision} git={git} />
      )}
    </div>
  );
}

/** One file's change, before and after as plain text; a binary file is only named. */
/** The line under the panel after a restore that left binary files as they are, or none. */
const keptNotice = (kept: string[]) =>
  kept.length ? m["synchist.kept"]({ files: kept.join(", ") }) : "";

function ChangeView({ c }: { c: SyncChange }) {
  const side = (label: string, text: string | null) => (
    <div className="min-w-0 flex-1">
      <div className="text-[10px] text-muted-foreground">{label}</div>
      <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-all rounded bg-muted/40 p-1 text-[10px]">
        {text ?? "—"}
      </pre>
    </div>
  );
  return (
    <div className="px-3 py-1">
      <div className="truncate font-mono text-[11px]">{c.file}</div>
      {c.binary ? (
        <div className="text-muted-foreground">{m["synchist.binary"]()}</div>
      ) : (
        <div className="flex gap-1">
          {side(m["synchist.before"](), c.before)}
          {side(m["synchist.after"](), c.after)}
        </div>
      )}
    </div>
  );
}

/** The workspace's sync history: its checkpoints and latest edits, each one restorable and openable. */
function SyncHistorySection({
  revision,
  git,
}: {
  revision: unknown;
  git: GitState | null;
}) {
  const workspace = cfg().workspace;
  const syncHistory = historyClient;
  const [hist, setHist] = useState<SyncHistory | null>(null);
  const [error, setError] = useState("");
  /** A failed read of the list, apart from a failed action: Retry only fits the first. */
  const [readError, setReadError] = useState("");
  const [notice, setNotice] = useState("");
  const offers = historyOffers(git);
  const [open, setOpen] = useState<string | null>(null);
  const [changes, setChanges] = useState<SyncChange[] | null>(null);

  const refresh = useCallback(async () => {
    if (!workspace) return;
    try {
      setHist(await historyClient.list(workspace));
      setReadError("");
    } catch (e) {
      setReadError((e as Error).message);
    }
  }, [workspace]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `revision` is the trigger, not an input.
  useEffect(() => {
    void refresh();
  }, [refresh, revision]);

  // A row opens on what it changed: an entry against the point before it, a checkpoint against now.
  const toggle = async (key: string, from: number, to: number) => {
    if (open === key) return setOpen(null);
    setOpen(key);
    setChanges(null);
    try {
      setChanges(await syncHistory.diff(workspace, from, to));
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const name = async () => {
    if (!offers.write) return;
    const n = await askText({
      title: m["synchist.namePrompt"](),
      placeholder: m["synchist.namePlaceholder"](),
      confirmLabel: m["synchist.nameConfirm"](),
      cancelLabel: m["common.cancel"](),
      validate: (v) => (v.trim() ? "" : m["synchist.nameEmpty"]()),
    });
    if (!n) return;
    await syncHistory
      .checkpoint(workspace, n.trim())
      .catch((e: Error) => setError(e.message));
    void refresh();
  };

  const restore = async (seq: number, point: string) => {
    if (!offers.write) return;
    const ok = await askConfirm({
      // Named by what it is and when, since a sequence number is nothing anyone has seen.
      title: m["synchist.restoreTitle"]({ point }),
      body: m["synchist.restoreBodyDevice"](),
      confirmLabel: m["synchist.restoreConfirm"](),
      cancelLabel: m["common.cancel"](),
      danger: true,
    });
    if (!ok) return;
    setNotice("");
    await syncHistory
      .restore(workspace, seq)
      .then((r) => setNotice(keptNotice(restoredKept(r))))
      .catch((e: Error) => setError(e.message));
    void refresh();
  };

  if (!workspace) return null;
  const head = hist?.head ?? 0;
  const row = (
    key: string,
    ts: number,
    label: string,
    from: number,
    to: number,
    seq: number,
    point: string,
    icon: ReactNode,
  ) => (
    <div key={key}>
      <div className="flex items-center">
        <RowButton
          className="min-w-0 flex-1"
          icon={icon}
          detail={time(ts)}
          trailing={
            open === key ? (
              <ChevronDown className="size-4 text-fg-3" />
            ) : (
              <ChevronRight className="size-4 text-fg-3" />
            )
          }
          onClick={() => void toggle(key, from, to)}
        >
          {label}
        </RowButton>
        {offers.write && (
          <Button
            variant="ghost"
            size="round"
            className="shrink-0"
            title={m["synchist.restore"]()}
            aria-label={m["synchist.restore"]()}
            onClick={() => void restore(seq, `${point}, ${time(ts)}`)}
          >
            <RotateCcw className="size-4" />
          </Button>
        )}
      </div>
      {open === key &&
        (changes === null ? null : changes.length === 0 ? (
          <p className="px-3.5 py-2 text-fg-2">{m["synchist.noChange"]()}</p>
        ) : (
          changes.map((c) => <ChangeView key={c.file} c={c} />)
        ))}
    </div>
  );

  return (
    <section className="flex min-h-0 flex-1 flex-col text-xs">
      <header className="flex shrink-0 items-center gap-1 px-2 py-1">
        <span className="min-w-0 flex-1 px-1.5 text-fg-3">
          {m["synchist.title"]({ workspace })}
        </span>
        {offers.write && (
          <Button size="sm" variant="ghost" onClick={() => void name()}>
            <Bookmark className="h-3.5 w-3.5" />
            {m["synchist.name"]()}
          </Button>
        )}
      </header>
      {!offers.write && (
        <div className="shrink-0 px-3 py-1 text-muted-foreground">
          {m["synchist.readOnly"]()}
        </div>
      )}
      {notice && (
        <div className="shrink-0 px-3 py-1 text-muted-foreground">{notice}</div>
      )}
      {readError && (
        <PanelError
          what={m["panel.what.history"]()}
          detail={readError}
          onRetry={() => void refresh()}
        />
      )}
      {(error || hist?.error) && (
        <div className="shrink-0 px-3 py-1 text-destructive">
          {error || m["synchist.noRoom"]({ error: hist?.error ?? "" })}
        </div>
      )}
      <RowGroup className="min-h-0 flex-1 overflow-y-auto">
        {hist && hist.checkpoints.length === 0 && hist.entries.length === 0 && (
          <p className="px-3.5 py-3 text-fg-2">{m["synchist.empty"]()}</p>
        )}
        {[...(hist?.checkpoints ?? [])]
          .reverse()
          .map((c) =>
            row(
              `c${c.seq}-${c.ts}`,
              c.ts,
              c.by === "auto" ? m["synchist.auto"]() : c.name,
              c.seq,
              head,
              c.seq,
              c.by === "auto" ? m["synchist.auto"]() : c.name,
              <Bookmark />,
            ),
          )}
        {(hist?.entries ?? []).map((e) =>
          row(
            `e${e.seq}`,
            e.ts,
            e.parked
              ? `${e.label} · ${m["synchist.parked"]({ count: e.parked })}`
              : e.label,
            e.seq - 1,
            e.seq,
            e.seq,
            e.label,
            <Dot />,
          ),
        )}
      </RowGroup>
      <p className="type-status shrink-0 px-3.5 py-2 text-fg-3">
        {m["synchist.footnote"]()}
      </p>
    </section>
  );
}
