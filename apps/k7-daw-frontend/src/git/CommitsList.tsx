// CommitsList.tsx — the History panel's second list: the repository's commits, newest first, older pages on demand.
// The first list (Edits) is the local history; this one is what was committed, and what a pull brought.

import { Button } from "@studio/ui/button";
import { ArrowUp, GitCommitHorizontal } from "@studio/ui/icons";
import { SectionHeading } from "@studio/ui/section-heading";
import { useCallback, useEffect, useReducer, useState } from "react";
import { m } from "../paraglide/messages.js";
import { getLocale } from "../paraglide/runtime.js";
import { explain, type GitTransport, git } from "./gitApi";
import { remoteName } from "./gitState";
import type { CommitInfo, GitState } from "./gitTypes";
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
  state = null,
}: {
  transport: GitTransport | null;
  workspace: string;
  /** Moves when the repository does (a commit, a pull): the list restarts at its head. */
  refreshKey: unknown;
  /** The repository's state, for the line over the list: branch, remote, what a pull would bring. */
  state?: GitState | null;
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

  // The newest `ahead` commits are the ones the remote has not got yet.
  const ahead =
    state?.mode === "connected"
      ? Math.min(state.ahead ?? 0, log.commits.length)
      : 0;
  const pushed =
    state?.mode === "connected" && state.remote
      ? `${remoteName(state.remote)} · ${state.branch ?? ""}`
      : null;
  const row = (c: CommitInfo, up: boolean) => (
    <li
      key={c.id}
      className="flex items-start gap-2 px-3 py-2 hover:bg-on-fill"
      title={c.id}
    >
      {up ? (
        <ArrowUp className="mt-0.5 size-3.5 shrink-0 text-primary-text" />
      ) : (
        <GitCommitHorizontal className="mt-0.5 size-3.5 shrink-0 text-fg-2" />
      )}
      <div className="min-w-0 flex-1">
        <div className="truncate">{c.subject}</div>
        <div className="truncate type-badge text-fg-2">
          {[c.author, c.device, stamp(c.time)].filter(Boolean).join(" · ")}
        </div>
      </div>
      <span className="shrink-0 type-badge text-fg-2 tabular-nums">
        {(c.files_changed === 1 ? m["git.files.one"] : m["git.files.other"])({
          n: c.files_changed,
        })}
      </span>
    </li>
  );
  return (
    <div className="min-h-0 flex-1 overflow-y-auto py-1 type-secondary">
      {state?.branch && (
        <p className="px-3 py-1.5 type-badge text-fg-2">
          {[
            state.branch,
            state.remote,
            state.behind
              ? (state.behind === 1
                  ? m["git.toPull.one"]
                  : m["git.toPull.other"])({ n: state.behind })
              : null,
          ]
            .filter(Boolean)
            .join(" · ")}
        </p>
      )}
      {error && <p className="px-3 py-2 text-error">{error}</p>}
      {log.commits.length === 0 && !error && (
        <div className="px-3 py-2 text-fg-2">
          <p>{m["git.commitsEmpty"]()}</p>
          <p className="type-status text-fg-3">{m["git.commitsEmptyNote"]()}</p>
        </div>
      )}
      {ahead > 0 && (
        <>
          <SectionHeading className="px-3 pt-1">
            {m["git.toPushGroup"]({ n: ahead })}
          </SectionHeading>
          <ul>{log.commits.slice(0, ahead).map((c) => row(c, true))}</ul>
        </>
      )}
      {pushed && log.commits.length > ahead && (
        <SectionHeading className="px-3 pt-2">{pushed}</SectionHeading>
      )}
      <ul>{log.commits.slice(ahead).map((c) => row(c, false))}</ul>
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
