// useGitState.ts — one workspace's git state, polled while the tab is visible, the acts that move it, and the
// page's word to the fetch (in front or away). A failed read keeps the last state (gitLock.ts).

import { useCallback, useEffect, useRef, useState } from "react";
import { pollWhileVisible } from "../visiblePoll";
import { focusBeat } from "./focusBeat";
import { explain, type GitTransport, git } from "./gitApi";
import { onGitMoved } from "./gitBus";
import { afterRead, lastSeen, remember } from "./gitLock";
import type { GitState, PullState } from "./gitTypes";

/** The state is a local read, so the bar asks at the pace the share's bar did. */
const POLL_MS = 5000;

/** How long a one-shot fetch is given before the card reads the state again. */
const FETCH_SETTLE_MS = 4000;

export type GitAct = "push" | "pull" | "abort";

export interface GitHandle {
  state: GitState | null;
  /** A refusal or failure of the last act or read, as a sentence. */
  failure: string | null;
  busy: GitAct | null;
  /** What the last pull came to, until the next act. */
  pulled: PullState | null;
  act: (what: GitAct) => Promise<void>;
  refresh: () => Promise<void>;
  say: (sentence: string | null) => void;
}

/** `live` polls and tells the fetch the page is in front; `once` reads at mount and when announced; `manual` only when asked. */
export type Watch = "live" | "once" | "manual";

export function useGitState(
  t: GitTransport | null,
  workspace: string,
  watch: Watch = "live",
): GitHandle {
  const [state, setState] = useState<GitState | null>(() =>
    lastSeen(workspace),
  );
  const [failure, setFailure] = useState<string | null>(null);
  const [busy, setBusy] = useState<GitAct | null>(null);
  const [pulled, setPulled] = useState<PullState | null>(null);
  const live = useRef(true);
  const seq = useRef(0);

  const refresh = useCallback(async () => {
    if (!t || !workspace) return;
    const mine = ++seq.current;
    try {
      const s = await git.state(t, workspace);
      if (!live.current || mine !== seq.current) return;
      const next = afterRead(lastSeen(workspace), { state: s });
      remember(workspace, next);
      setState(next);
    } catch (error) {
      if (!live.current || mine !== seq.current) return;
      const next = afterRead(lastSeen(workspace), { error });
      remember(workspace, next);
      setState(next);
    }
  }, [t, workspace]);

  useEffect(() => {
    live.current = true;
    setState(lastSeen(workspace));
    if (watch !== "manual") void refresh();
    const stop =
      watch === "live" ? pollWhileVisible(refresh, POLL_MS) : () => {};
    const heard = onGitMoved(workspace, () => void refresh());
    return () => {
      live.current = false;
      stop();
      heard();
    };
  }, [refresh, workspace, watch]);

  // The fetch runs while the page says it is in front; the connected state starts it (a connect ends a task without a remote).
  const connected = state?.mode === "connected";
  useEffect(() => {
    if (!t || !workspace || !connected || watch !== "live") return;
    return focusBeat(
      (front, opened) => git.focus(t, workspace, front, opened),
      POLL_MS,
    );
  }, [t, workspace, connected, watch]);

  // A card reads once, so the page asks for one fetch on opening and reads again once it has had time to land.
  useEffect(() => {
    if (!t || !workspace || !connected || watch !== "once") return;
    void git.focus(t, workspace, true, true).catch(() => undefined);
    const id = window.setTimeout(() => void refresh(), FETCH_SETTLE_MS);
    return () => window.clearTimeout(id);
  }, [t, workspace, connected, watch, refresh]);

  const act = useCallback(
    async (what: GitAct) => {
      if (!t || busy) return;
      setBusy(what);
      setFailure(null);
      setPulled(null);
      try {
        if (what === "push") await git.push(t, workspace);
        else if (what === "abort") await git.abort(t, workspace);
        else setPulled((await git.pull(t, workspace)).state);
      } catch (e) {
        setFailure(explain(e));
      } finally {
        setBusy(null);
        await refresh();
      }
    },
    [t, workspace, busy, refresh],
  );

  return { state, failure, busy, pulled, act, refresh, say: setFailure };
}
