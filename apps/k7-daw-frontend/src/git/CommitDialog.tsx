// CommitDialog.tsx — Commit & Push as one act: the changed files, a message, then a step list that ends in Done.
// What a reader gets wrong: a push the remote refuses as behind is a state of this dialog (pull first), not an error.

import { Button } from "@studio/ui/button";
import { Chip } from "@studio/ui/chip";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@studio/ui/dialog";
import { Loader2 } from "@studio/ui/icons";
import { Input } from "@studio/ui/input";
import { useEffect, useState } from "react";
import { m } from "../paraglide/messages.js";
import { CommitSteps } from "./CommitSteps";
import {
  type CommitPhase,
  commitView,
  formatSize,
  identityReady,
  identityToSave,
  stepRows,
} from "./commitState";
import {
  explain,
  GitError,
  type GitTransport,
  git,
  type Identity,
} from "./gitApi";
import type { GitState, StatusEntry } from "./gitTypes";
import { saveIdentity } from "./identity";

const CHANGE = {
  added: { key: "git.changeAdded", tone: "new" },
  modified: { key: "git.changeModified", tone: "changed" },
  removed: { key: "git.changeRemoved", tone: "removed" },
  renamed: { key: "git.changeRenamed", tone: "renamed" },
} as const;

export function CommitDialog({
  transport: t,
  workspace,
  open,
  onClose,
  onDone,
}: {
  transport: GitTransport | null;
  workspace: string;
  open: boolean;
  onClose: () => void;
  onDone: () => void;
}) {
  const [entries, setEntries] = useState<StatusEntry[] | null>(null);
  const [state, setState] = useState<GitState | null>(null);
  const [message, setMessage] = useState("");
  const [who, setWho] = useState<Identity | null>(null);
  const [prefilled, setPrefilled] = useState<Identity | null>(null);
  const [phase, setPhase] = useState<CommitPhase>({ kind: "ready" });
  const [error, setError] = useState("");

  useEffect(() => {
    if (!open || !t) return;
    let live = true;
    setEntries(null);
    setError("");
    setPhase({ kind: "ready" });
    git
      .status(t, workspace)
      .then((r) => live && setEntries(r.entries))
      .catch((e) => live && setError(explain(e)));
    git
      .state(t, workspace)
      .then((s) => live && setState(s))
      .catch(() => live && setState(null));
    git
      .identity(t)
      .then((id) => {
        if (!live) return;
        setWho(id);
        setPrefilled(id);
      })
      .catch(() => live && setWho({ name: "", email: "" }));
    return () => {
      live = false;
    };
  }, [open, t, workspace]);

  const view = commitView(entries ?? [], message);
  const connected = state?.mode === "connected";
  const pushOnly =
    view.block === "noChanges" && connected && (state?.ahead ?? 0) > 0;
  const plan = { commit: !pushOnly, push: connected };
  const signed = identityReady(who);
  const busy = phase.kind === "running";
  // Every state of the act but ready has left the form behind: only the step list and its way out show.
  const started = phase.kind !== "ready";
  const finished = phase.kind === "done" || phase.kind === "pullFirst";
  const retryFrom = phase.kind === "failed" ? phase.step : null;
  const ready =
    pushOnly || retryFrom === "push" || (view.block === null && signed);
  const ok = ready && !busy && Boolean(t) && !(entries === null && !error);

  const go = async () => {
    if (!t || !ok) return;
    setError("");
    try {
      if (plan.commit && retryFrom !== "push") {
        setPhase({ kind: "running", step: "commit" });
        const save = who ? identityToSave(prefilled, who) : null;
        if (save) await saveIdentity(t, save);
        await git.commit(
          t,
          workspace,
          message.trim(),
          view.leftOut.map((e) => e.path),
        );
        setMessage("");
        onDone();
      }
    } catch (e) {
      setPhase({ kind: "failed", step: "commit", error: explain(e) });
      return;
    }
    if (!plan.push) return setPhase({ kind: "done" });
    try {
      setPhase({ kind: "running", step: "push" });
      await git.push(t, workspace);
      onDone();
      setPhase({ kind: "done" });
    } catch (e) {
      onDone();
      if (e instanceof GitError && e.code === "behind")
        setPhase({ kind: "pullFirst" });
      else setPhase({ kind: "failed", step: "push", error: explain(e) });
    }
  };

  return (
    <Dialog open={open} onOpenChange={(o) => !busy && !o && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{m["git.commitPushTitle"]()}</DialogTitle>
          <DialogDescription>{m["git.commitBody"]()}</DialogDescription>
        </DialogHeader>
        <ul
          aria-label={m["git.files"]()}
          className="max-h-56 overflow-y-auto rounded-xl border border-(--k7-hairline) type-secondary"
        >
          {entries === null && !error && (
            <li className="px-3 py-2 text-fg-2">
              <Loader2 className="mr-1 inline size-3 animate-spin" />
              {m["git.reading"]()}
            </li>
          )}
          {entries?.length === 0 && (
            <li className="px-3 py-2 text-fg-2">{m["git.noChanges"]()}</li>
          )}
          {view.rows.map((e) => (
            <li
              key={e.path}
              className="flex items-center gap-2 border-b border-(--k7-hairline) px-3 py-1.5 last:border-b-0"
            >
              <Chip variant="status" tone={CHANGE[e.change].tone}>
                {m[CHANGE[e.change].key]()}
              </Chip>
              <span
                className="min-w-0 flex-1 truncate type-mono-value"
                title={e.from ? `${e.from} → ${e.path}` : e.path}
              >
                {e.path}
              </span>
              {e.lfs && (
                <Chip
                  variant="status"
                  tone="renamed"
                  title={m["git.lfsHint"]()}
                >
                  {m["git.lfs"]()}
                </Chip>
              )}
              <span className="w-16 shrink-0 text-right tabular-nums text-fg-2">
                {formatSize(e.size)}
              </span>
            </li>
          ))}
        </ul>
        {view.leftOut.map((e) => (
          <p key={e.path} className="type-secondary text-fg-2">
            {m["git.leftOut"]({ name: e.path, size: formatSize(e.size) })}
          </p>
        ))}
        {!started && who && !signed && plan.commit && (
          <div className="flex flex-col gap-2">
            <p className="type-secondary text-fg-2">
              {m["git.identityNeeded"]()}
            </p>
            <Input
              aria-label={m["git.identityName"]()}
              placeholder={m["git.identityName"]()}
              value={who.name}
              onChange={(e) => setWho({ ...who, name: e.target.value })}
            />
            <Input
              aria-label={m["git.identityEmail"]()}
              placeholder={m["git.identityEmail"]()}
              value={who.email}
              onChange={(e) => setWho({ ...who, email: e.target.value })}
            />
          </div>
        )}
        {!started && plan.commit && (
          <textarea
            aria-label={m["git.message"]()}
            placeholder={m["git.message"]()}
            className="min-h-20 rounded-xl border border-(--k7-hairline-strong) bg-transparent px-3 py-2 text-sm"
            value={message}
            onChange={(e) => setMessage(e.target.value)}
          />
        )}
        {started && <CommitSteps rows={stepRows(phase, plan)} />}
        {error && (
          <p role="alert" className="whitespace-pre-wrap text-error text-xs">
            {error}
          </p>
        )}
        <DialogFooter>
          <Button variant="ghost" disabled={busy} onClick={onClose}>
            {finished ? m["common.close"]() : m["common.cancel"]()}
          </Button>
          {!finished && (
            <Button disabled={!ok} onClick={() => void go()}>
              {busy && <Loader2 className="mr-1 size-3 animate-spin" />}
              {retryFrom ? m["git.tryAgain"]() : m["git.commitPush"]()}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
