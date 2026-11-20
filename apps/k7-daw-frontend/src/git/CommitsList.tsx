// CommitsList.tsx — the History panel's second list: the repository's commits, newest first, older pages on demand.
// The first list (Edits) is the local history; this one is what was committed, and what a pull brought.

import { Button } from "@studio/ui/button";
import { GitCommitHorizontal } from "@studio/ui/icons";
import { useCallback, useEffect, useReducer, useState } from "react";
import { m } from "../paraglide/messages.js";
import { getLocale } from "../paraglide/runtime.js";
import { explain, type GitTransport, git } from "./gitApi";
import { cursor, emptyLog, logAfter, PAGE } from "./logState";

const stamp = (sec: number) =>
  new Date(sec * 1000).toLocaleString(getLocale(), {
    dateStyle: "medium",
    timeStyle: "short",
  });

export function CommitsList({
  transport: t,
  workspace,
  refreshKey,
}: {
  transport: GitTransport | null;
  workspace: string;
  /** Moves when the repository does (a commit, a pull): the list restarts at its head. */
  refreshKey: unknown;
}) {
  const [log, dispatch] = useReducer(logAfter, emptyLog);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  // biome-ignore lint/correctness/useExhaustiveDependencies: `refreshKey` is the trigger, not an input.
  useEffect(() => {
    if (!t || !workspace) return;
    let live = true;
    git
      .log(t, workspace, PAGE)
      .then((r) => {
        if (!live) return;
        dispatch({ type: "first", commits: r.commits });
        setError("");
      })
      .catch((e) => live && setError(explain(e)));
    return () => {
      live = false;
    };
  }, [t, workspace, refreshKey]);

  const more = useCallback(async () => {
    if (!t) return;
    setLoading(true);
    try {
      const r = await git.log(t, workspace, PAGE, cursor(log));
      dispatch({ type: "more", commits: r.commits });
    } catch (e) {
      setError(explain(e));
    } finally {
      setLoading(false);
    }
  }, [t, workspace, log]);

  return (
    <div className="min-h-0 flex-1 overflow-y-auto py-1 type-secondary">
      {error && <p className="px-3 py-2 text-error">{error}</p>}
      {log.commits.length === 0 && !error && (
        <p className="px-3 py-2 text-fg-2">{m["git.commitsEmpty"]()}</p>
      )}
      <ul>
        {log.commits.map((c) => (
          <li
            key={c.id}
            className="flex items-start gap-2 px-3 py-2 hover:bg-on-fill"
            title={c.id}
          >
            <GitCommitHorizontal className="mt-0.5 size-3.5 shrink-0 text-fg-2" />
            <div className="min-w-0 flex-1">
              <div className="truncate">{c.subject}</div>
              <div className="truncate type-badge text-fg-2">
                {[c.author, c.device, stamp(c.time)]
                  .filter(Boolean)
                  .join(" · ")}
              </div>
            </div>
            <span className="shrink-0 type-badge text-fg-2 tabular-nums">
              {(c.files_changed === 1
                ? m["git.files.one"]
                : m["git.files.other"])({
                n: c.files_changed,
              })}
            </span>
          </li>
        ))}
      </ul>
      {!log.end && log.commits.length > 0 && (
        <div className="px-3 py-2">
          <Button
            size="xs"
            variant="ghost"
            disabled={loading}
            onClick={() => void more()}
          >
            {m["git.older"]()}
          </Button>
        </div>
      )}
    </div>
  );
}
