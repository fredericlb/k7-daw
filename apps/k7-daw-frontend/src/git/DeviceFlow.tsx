// DeviceFlow.tsx — GitHub's device flow as one screen: a code to type on github.com while k7 polls for the answer.
// What a reader gets wrong: polling stops with the screen and while the page is hidden, and the device code never
// reaches the page; only the short user code does.

import { Button, buttonVariants } from "@studio/ui/button";
import { Check, Copy, ExternalLink } from "@studio/ui/icons";
import { Spinner } from "@studio/ui/spinner";
import { useEffect, useReducer, useRef, useState } from "react";
import { m } from "../paraglide/messages.js";
import { copyText } from "./copyText";
import { explain, type GitTransport, git } from "./gitApi";
import { signInAfter, startingSignIn } from "./githubState";

export function DeviceFlow({
  t,
  scopes,
  why,
  onDone,
  onCancel,
}: {
  t: GitTransport;
  scopes: string[];
  /** One line on why this asks again, for the permission step. */
  why?: string;
  onDone: (login: string, keyNote?: string) => void;
  onCancel: () => void;
}) {
  const [state, dispatch] = useReducer(signInAfter, startingSignIn);
  const [attempt, setAttempt] = useState(0);
  const [copied, setCopied] = useState(false);
  const live = useRef({ onDone });
  live.current = { onDone };

  // biome-ignore lint/correctness/useExhaustiveDependencies: `attempt` restarts the flow; `scopes` is a constant of the caller.
  useEffect(() => {
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const fail = (e: unknown) =>
      !stopped && dispatch({ type: "failed", message: explain(e) });
    const poll = async (interval: number) => {
      if (stopped) return;
      if (document.hidden) {
        // A hidden page stops asking; it asks again at once when it is back.
        document.addEventListener(
          "visibilitychange",
          () => void poll(interval),
          {
            once: true,
          },
        );
        return;
      }
      try {
        const answer = await git.githubPoll(t);
        if (stopped) return;
        dispatch({ type: "polled", answer });
        if (answer.state === "ok")
          live.current.onDone(answer.login ?? "", answer.keyNote);
        else if (answer.state === "pending" || answer.state === "slow_down")
          timer = setTimeout(
            () => void poll(answer.interval ?? interval),
            (answer.interval ?? interval) * 1000,
          );
      } catch (e) {
        fail(e);
      }
    };
    git
      .githubStart(t, scopes)
      .then((code) => {
        if (stopped) return;
        dispatch({ type: "started", code });
        timer = setTimeout(
          () => void poll(code.interval),
          code.interval * 1000,
        );
      })
      .catch(fail);
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  }, [t, attempt]);

  const retry = () => {
    dispatch({ type: "again" });
    setAttempt((n) => n + 1);
  };
  const problem =
    state.phase === "expired"
      ? m["git.gh.expired"]()
      : state.phase === "denied"
        ? m["git.gh.denied"]()
        : state.phase === "failed"
          ? state.message
          : "";
  return (
    <div className="flex flex-col gap-3">
      {why && <p className="text-fg-2 type-secondary">{why}</p>}
      {problem ? (
        <>
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
        </>
      ) : state.phase === "waiting" ? (
        <>
          <p className="type-label">{m["git.gh.codeTitle"]()}</p>
          <code className="select-all rounded-xl border border-(--k7-hairline-strong) bg-(--k7-row) px-4 py-3 text-center type-mono-value text-2xl tracking-widest">
            {state.code}
          </code>
          <div className="flex flex-wrap gap-2">
            <Button
              size="sm"
              variant="secondary"
              onClick={async () => setCopied(await copyText(state.code))}
            >
              {copied ? (
                <Check className="mr-1 size-3" />
              ) : (
                <Copy className="mr-1 size-3" />
              )}
              {copied ? m["git.keyCopied"]() : m["git.gh.copyCode"]()}
            </Button>
            <a
              href={state.uri}
              target="_blank"
              rel="noreferrer"
              className={buttonVariants({ size: "sm" })}
            >
              <ExternalLink className="mr-1 size-3" />
              {m["git.gh.open"]()}
            </a>
          </div>
          <p className="text-fg-2 type-secondary" role="status">
            <Spinner size="xs" inherit className="mr-1 inline" />
            {m["git.gh.waiting"]()}
          </p>
          <Button
            size="sm"
            variant="ghost"
            className="self-start"
            onClick={onCancel}
          >
            {m["common.cancel"]()}
          </Button>
        </>
      ) : (
        <p className="text-fg-2 type-secondary" role="status">
          <Spinner size="xs" inherit className="mr-1 inline" />
          {m["git.gh.starting"]()}
        </p>
      )}
    </div>
  );
}
