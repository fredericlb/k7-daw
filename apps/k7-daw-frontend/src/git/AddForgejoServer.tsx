// AddForgejoServer.tsx — "Add a Forgejo server": an address, what a check read from it, then Add.
// What a reader gets wrong: no client id is typed unless the server's admin turned off Forgejo's built-in clients (docs/git.md).

import { Button } from "@studio/ui/button";
import { Check, Copy } from "@studio/ui/icons";
import { Input } from "@studio/ui/input";
import { Spinner } from "@studio/ui/spinner";
import { type ReactNode, useReducer, useState } from "react";
import { m } from "../paraglide/messages.js";
import { copyText } from "./copyText";
import {
  addAfter,
  canAdd,
  probeOf,
  shortVersion,
  sshPort,
  startAdding,
} from "./forgejoState";
import type { ForgeProbe } from "./forgeState";
import { explain, type GitTransport, git } from "./gitApi";

function CopyLine({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="flex items-center gap-2">
      <code className="min-w-0 flex-1 select-all truncate rounded-xl border border-(--k7-hairline-strong) bg-(--k7-row) px-3 py-2 type-mono-value">
        {text}
      </code>
      <Button
        size="sm"
        variant="secondary"
        type="button"
        onClick={async () => setCopied(await copyText(text))}
      >
        {copied ? (
          <Check className="mr-1 size-3" />
        ) : (
          <Copy className="mr-1 size-3" />
        )}
        {copied ? m["git.keyCopied"]() : m["git.fj.copy"]()}
      </Button>
    </div>
  );
}

const Note = ({ children, bad }: { children: ReactNode; bad?: boolean }) => (
  <p
    role={bad ? "alert" : undefined}
    className={`type-secondary ${bad ? "text-error" : "text-fg-2"}`}
  >
    {children}
  </p>
);

/** What the check read: the version, large files, whether the web address answers. */
export function ProbeFacts({ probe }: { probe: ForgeProbe }) {
  return (
    <div className="flex flex-col gap-1">
      <Note>{m["git.fj.found"]({ version: shortVersion(probe.version) })}</Note>
      <Note bad={!probe.lfs}>
        {probe.lfs ? m["git.fj.lfsOn"]() : m["git.fj.lfsOff"]()}
      </Note>
      <Note bad={!probe.rootAnswers}>
        {probe.rootAnswers
          ? m["git.fj.rootOk"]()
          : m["git.fj.rootBad"]({ root: probe.rootUrl })}
      </Note>
    </div>
  );
}

export function AddForgejoServer({
  t,
  onAdded,
  onCancel,
}: {
  t: GitTransport;
  onAdded: (id: string) => void;
  onCancel: () => void;
}) {
  const [state, dispatch] = useReducer(addAfter, startAdding);
  const [url, setUrl] = useState("");
  const [clientId, setClientId] = useState("");
  const [host, setHost] = useState("");
  const [port, setPort] = useState("");
  const probe = probeOf(state);
  const busy = state.phase === "checking" || state.phase === "adding";

  const check = async () => {
    dispatch({ type: "check" });
    try {
      dispatch({ type: "probed", probe: await git.forgeProbe(t, url.trim()) });
    } catch (e) {
      dispatch({ type: "failed", message: explain(e) });
    }
  };
  const add = async () => {
    const p = sshPort(port);
    if (!probe || p === "bad") return;
    dispatch({ type: "add" });
    try {
      const done = await git.forgeAdd(t, {
        url: probe.url,
        clientId: clientId.trim() || undefined,
        sshHost: host.trim() || undefined,
        sshPort: p,
      });
      onAdded(done.id);
    } catch (e) {
      dispatch({ type: "failed", message: explain(e) });
    }
  };

  return (
    <div className="flex flex-col gap-3 px-3.5 py-2.5">
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (url.trim() && !busy) void check();
        }}
      >
        <Input
          autoFocus
          aria-label={m["git.fj.address"]()}
          placeholder={m["git.fj.addressPlaceholder"]()}
          autoComplete="off"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
        />
        <Button
          type="submit"
          variant="secondary"
          disabled={!url.trim() || busy}
        >
          {state.phase === "checking" && (
            <Spinner size="xs" inherit className="mr-1" />
          )}
          {state.phase === "checking"
            ? m["git.fj.checking"]()
            : m["git.fj.check"]()}
        </Button>
      </form>
      {probe && (
        <>
          <ProbeFacts probe={probe} />
          {/* A server whose admin turned off the built-in clients takes an application registered on it instead. */}
          <details className="type-secondary text-fg-2">
            <summary className="cursor-pointer">
              {m["git.fj.registerTitle"]()}
            </summary>
            <div className="mt-2 flex flex-col gap-2">
              <Note>{m["git.fj.registerHow"]()}</Note>
              {probe.redirects.map((r) => (
                <CopyLine key={r} text={r} />
              ))}
              <Input
                aria-label={m["git.fj.clientId"]()}
                placeholder={m["git.fj.clientId"]()}
                autoComplete="off"
                value={clientId}
                onChange={(e) => setClientId(e.target.value)}
              />
            </div>
          </details>
          <details className="type-secondary text-fg-2">
            <summary className="cursor-pointer">
              {m["git.fj.sshMore"]()}
            </summary>
            <div className="mt-2 flex gap-2">
              <Input
                aria-label={m["git.fj.sshHost"]()}
                placeholder={m["git.fj.sshHost"]()}
                value={host}
                onChange={(e) => setHost(e.target.value)}
              />
              <Input
                aria-label={m["git.fj.sshPort"]()}
                placeholder={m["git.fj.sshPort"]()}
                inputMode="numeric"
                value={port}
                onChange={(e) => setPort(e.target.value)}
              />
            </div>
          </details>
        </>
      )}
      {state.phase === "failed" && <Note bad>{state.message}</Note>}
      <div className="flex gap-2">
        {probe && (
          <Button
            size="sm"
            disabled={!canAdd(port) || busy}
            onClick={() => void add()}
          >
            {state.phase === "adding" && (
              <Spinner size="xs" inherit className="mr-1" />
            )}
            {state.phase === "adding"
              ? m["git.fj.adding"]()
              : m["git.fj.addServer"]()}
          </Button>
        )}
        <Button size="sm" variant="ghost" disabled={busy} onClick={onCancel}>
          {m["common.cancel"]()}
        </Button>
      </div>
    </div>
  );
}
