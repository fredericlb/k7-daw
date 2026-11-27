// ForgejoSignIn.tsx — one Forgejo sign-in: the server's page in a browser tab, or iOS's sheet, while k7 polls the front.
// What a reader gets wrong: polling goes on while this page is hidden, since the person is in the other tab; the front says each end once.

import { Button, buttonVariants } from "@studio/ui/button";
import { ExternalLink } from "@studio/ui/icons";
import { Spinner } from "@studio/ui/spinner";
import { useEffect, useReducer, useRef, useState } from "react";
import { m } from "../paraglide/messages.js";
import { POLL_MS, signInAfter, startingSignIn } from "./forgejoState";
import { openSheet, type SignInWay, signInWay } from "./forgeOpener";
import { explain, type GitTransport, git } from "./gitApi";

export function ForgejoSignIn({
  t,
  server,
  onDone,
  onCancel,
  way = signInWay,
}: {
  t: GitTransport;
  server: { id: string; name: string };
  onDone: (login: string, keyNote?: string) => void;
  onCancel: () => void;
  way?: SignInWay;
}) {
  const [state, dispatch] = useReducer(signInAfter, startingSignIn);
  const [attempt, setAttempt] = useState(0);
  const live = useRef({ onDone });
  live.current = { onDone };
  // A sheet the person closed changes nothing: the poll says how the sign-in ended, or Cancel ends it.
  const sheet = (url: string) => void openSheet(url).catch(() => {});

  // biome-ignore lint/correctness/useExhaustiveDependencies: `attempt` restarts the flow; `server.id` names it; `way` is fixed per page.
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
      .forgeStart(t, server.id, way.redirect)
      .then((start) => {
        if (stopped) return;
        dispatch({ type: "started", start });
        // The sheet opens at once: the person already asked to sign in, and the button opens it again once closed.
        if (way.open === "sheet") sheet(start.authorizeUrl);
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
        {way.open === "sheet" ? (
          <Button
            size="sm"
            className="self-start"
            onClick={() => sheet(state.url)}
          >
            <ExternalLink className="mr-1 size-3" />
            {m["git.fj.open"]({ server: server.name })}
          </Button>
        ) : (
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
        )}
        <p className="text-fg-2 type-secondary" role="status">
          <Spinner size="xs" inherit className="mr-1 inline" />
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
      <Spinner size="xs" inherit className="mr-1 inline" />
      {m["git.fj.starting"]({ server: server.name })}
    </p>
  );
}
