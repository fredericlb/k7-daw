// ForgejoSignIn.tsx — one Forgejo sign-in: a button that opens the server's page in a browser tab, while k7 polls the front.
// What a reader gets wrong: polling goes on while this page is hidden, since the person is in the other tab; the front says each end once.

import { Button, buttonVariants } from "@studio/ui/button";
import { ExternalLink, Loader2 } from "@studio/ui/icons";
import { useEffect, useReducer, useRef, useState } from "react";
import { m } from "../paraglide/messages.js";
import { POLL_MS, signInAfter, startingSignIn } from "./forgejoState";
import { explain, type GitTransport, git } from "./gitApi";

export function ForgejoSignIn({
  t,
  server,
  onDone,
  onCancel,
}: {
  t: GitTransport;
  server: { id: string; name: string };
  onDone: (login: string, keyNote?: string) => void;
  onCancel: () => void;
}) {
  const [state, dispatch] = useReducer(signInAfter, startingSignIn);
  const [attempt, setAttempt] = useState(0);
  const live = useRef({ onDone });
  live.current = { onDone };

  // biome-ignore lint/correctness/useExhaustiveDependencies: `attempt` restarts the flow; `server.id` names it.
  useEffect(() => {
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const fail = (e: unknown) =>
      !stopped && dispatch({ type: "failed", message: explain(e) });
    const poll = async () => {
      try {
        const answer = await git.forgePoll(t, server.id);
        if (stopped) return;
        dispatch({ type: "polled", answer });
        if (answer.state === "ok")
          live.current.onDone(answer.login ?? "", answer.keyNote);
        else if (answer.state === "pending")
          timer = setTimeout(() => void poll(), POLL_MS);
      } catch (e) {
        fail(e);
      }
    };
    git
      .forgeStart(t, server.id)
      .then((start) => {
        if (stopped) return;
        dispatch({ type: "started", start });
        timer = setTimeout(() => void poll(), POLL_MS);
      })
      .catch(fail);
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  }, [t, server.id, attempt]);

  const retry = () => {
    dispatch({ type: "again" });
    setAttempt((n) => n + 1);
  };
  const problem =
    state.phase === "expired"
      ? m["git.fj.expired"]()
      : state.phase === "denied"
        ? m["git.fj.denied"]({ server: server.name })
        : state.phase === "failed"
          ? m["git.fj.failed"]({ error: state.message })
          : "";
  if (problem)
    return (
      <div className="flex flex-col gap-3">
        <p role="alert" className="text-error type-secondary">
          {problem}
        </p>
        <div className="flex gap-2">
          <Button size="sm" onClick={retry}>
            {m["git.gh.tryAgain"]()}
          </Button>
          <Button size="sm" variant="ghost" onClick={onCancel}>
            {m["common.cancel"]()}
          </Button>
        </div>
      </div>
    );
  if (state.phase === "waiting")
    return (
      <div className="flex flex-col gap-3">
        <a
          href={state.url}
          target="_blank"
          rel="noreferrer"
          className={buttonVariants({ size: "sm" })}
          style={{ alignSelf: "flex-start" }}
        >
          <ExternalLink className="mr-1 size-3" />
          {m["git.fj.open"]({ server: server.name })}
        </a>
        <p className="text-fg-2 type-secondary" role="status">
          <Loader2 className="mr-1 inline size-3 animate-spin" />
          {m["git.fj.waiting"]({ server: server.name })}
        </p>
        <Button
          size="sm"
          variant="ghost"
          className="self-start"
          onClick={onCancel}
        >
          {m["common.cancel"]()}
        </Button>
      </div>
    );
  return (
    <p className="text-fg-2 type-secondary" role="status">
      <Loader2 className="mr-1 inline size-3 animate-spin" />
      {m["git.fj.starting"]({ server: server.name })}
    </p>
  );
}
