// CommitDialog.tsx — Commit & Push as one act: a message, the changed files, then a Commit and a Push step.
// What a reader gets wrong: a push the remote refuses as behind is a state of this dialog (pull first), not an error.

import { showToast } from "@studio/project-shell/toast";
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
import { Info, Loader2 } from "@studio/ui/icons";
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
import { remoteName } from "./gitState";
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
  onConflict,
}: {
  transport: GitTransport | null;
  workspace: string;
  open: boolean;
  onClose: () => void;
  onDone: () => void;
  /** A pull from this dialog diverged: the host opens the merge screen. */
  onConflict: () => void;
}) {
  const [entries, setEntries] = useState<StatusEntry[] | null>(null);
  const [state, setState] = useState<GitState | null>(null);
  const [message, setMessage] = useState("");
  const [who, setWho] = useState<Identity | null>(null);
  const [prefilled, setPrefilled] = useState<Identity | null>(null);
  const [phase, setPhase] = useState<CommitPhase>({ kind: "ready" });
  const [error, setError] = useState("");
  const [pulling, setPulling] = useState(false);
  const [sentMessage, setSentMessage] = useState("");

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
  const target = state?.remote
    ? `${remoteName(state.remote)} · ${state.branch ?? ""}`
    : "";
  const sentFiles = view.rows.length - view.leftOut.length;
  const count = <T,>(one: T, other: T, n: number): T => (n === 1 ? one : other);
  const fileCount = (n: number) =>
    count(m["git.files.one"], m["git.files.other"], n)({ n });
  // Every state of the act but ready has left the form behind: only the step list and its way out show.
  const started = phase.kind !== "ready";
  const finished = phase.kind === "pullFirst";
  const nothing = entries?.length === 0 && !pushOnly && !started;
  const retryFrom = phase.kind === "failed" ? phase.step : null;
  const ready =
    pushOnly || retryFrom === "push" || (view.block === null && signed);
  const ok = ready && !busy && Boolean(t) && !(entries === null && !error);

  const go = async () => {
    if (!t || !ok) return;
    setError("");
    if (retryFrom !== "push") setSentMessage(message.trim());
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
    const finish = () => {
      const said = fileCount(sentFiles);
      if (plan.push)
        showToast({
          title: plan.commit ? m["git.doneCommitPush"]() : m["git.donePush"](),
          message: plan.commit
            ? `${said} ${m["git.pushDetail"]({ target })}`
            : m["git.pushDetail"]({ target }),
        });
      else showToast({ title: m["git.doneCommit"](), message: said });
      onClose();
    };
    if (!plan.push) return finish();
    try {
      setPhase({ kind: "running", step: "push" });
      await git.push(t, workspace);
      onDone();
      finish();
    } catch (e) {
      onDone();
      if (e instanceof GitError && e.code === "behind") {
        git
          .state(t, workspace)
          .then(setState)
          .catch(() => undefined);
        setPhase({ kind: "pullFirst" });
      } else setPhase({ kind: "failed", step: "push", error: explain(e) });
    }
  };

  const pullNow = async () => {
    if (!t) return;
    setPulling(true);
    try {
      const r = await git.pull(t, workspace);
      onDone();
      onClose();
      if (r.state === "conflicts") onConflict();
    } catch (e) {
      setError(explain(e));
    } finally {
      setPulling(false);
    }
  };

  const sending = (): string => {
    const commits = (state?.ahead ?? 0) + (plan.commit ? 1 : 0);
    const a = count(
      m["git.commitsN.one"],
      m["git.commitsN.other"],
      commits,
    )({
      n: commits,
    });
    const b = count(
      m["git.largeN.one"],
      m["git.largeN.other"],
      view.lfs,
    )({
      n: view.lfs,
    });
    return m["git.sending"]({
      what: view.lfs > 0 ? m["git.sendingAnd"]({ a, b }) : a,
    });
  };
  const details = {
    commit: `${sentMessage} · ${fileCount(sentFiles)}`,
    push:
      phase.kind === "running" && phase.step === "push"
        ? sending()
        : m["git.pushDetail"]({ target }),
  };
  const n = view.rows.length;
  const title = nothing
    ? m["git.commitPushTitle"]()
    : (started
        ? count(m["git.commitTitleRun.one"], m["git.commitTitleRun.other"], n)
        : count(m["git.commitTitleAsk.one"], m["git.commitTitleAsk.other"], n))(
        {
          n,
        },
      );

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>
            {started
              ? m["git.stepsIntro"]()
              : connected
                ? m["git.commitBodyPush"]({ target })
                : m["git.commitBody"]()}
          </DialogDescription>
        </DialogHeader>
        {nothing ? (
          <div className="flex flex-col items-center gap-1 px-3 py-6 text-center">
            <p className="type-body">{m["git.noChanges"]()}</p>
            <p className="type-secondary text-fg-2">
              {m["git.noChangesNote"]()}
            </p>
          </div>
        ) : (
          !started && (
            <>
              {plan.commit && (
                <div className="flex flex-col gap-1.5">
                  <label htmlFor="commit-message" className="type-label">
                    {m["git.message"]()}
                  </label>
                  <Input
                    id="commit-message"
                    placeholder={m["git.messagePlaceholder"]()}
                    value={message}
                    onChange={(e) => setMessage(e.target.value)}
                  />
                </div>
              )}
              {who && !signed && plan.commit && (
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
              <p className="type-caps text-fg-3">
                {view.lfs > 0
                  ? m["git.filesHeadingLarge"]({
                      files: fileCount(n),
                      large: view.lfs,
                    })
                  : m["git.filesHeading"]({ files: fileCount(n) })}
              </p>
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
                <p
                  key={e.path}
                  role="note"
                  className="type-secondary flex items-start gap-2 rounded-xl border border-(--k7-warn-line) bg-(--k7-warn-fill) px-3 py-2 text-fg-2"
                >
                  <Info className="mt-0.5 size-4 shrink-0 text-warn" />
                  {m["git.leftOut"]({
                    name: e.path,
                    size: formatSize(e.size),
                  })}
                </p>
              ))}
            </>
          )
        )}
        {started && (
          <CommitSteps rows={stepRows(phase, plan)} details={details} />
        )}
        {error && (
          <p role="alert" className="whitespace-pre-wrap text-error text-xs">
            {error}
          </p>
        )}
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            {finished || nothing ? m["common.close"]() : m["common.cancel"]()}
          </Button>
          {finished && (state?.behind ?? 0) > 0 && (
            <Button disabled={pulling} onClick={() => void pullNow()}>
              {pulling && <Loader2 className="mr-1 size-3 animate-spin" />}
              {m["git.pullN"]({ n: state?.behind ?? 0 })}
            </Button>
          )}
          {!finished && !nothing && (
            <Button disabled={!ok} onClick={() => void go()}>
              {busy && <Loader2 className="mr-1 size-3 animate-spin" />}
              {busy
                ? phase.kind === "running" && phase.step === "push"
                  ? m["git.stepPushing"]()
                  : m["git.stepCommitting"]()
                : retryFrom
                  ? m["git.tryAgain"]()
                  : m["git.commitPush"]()}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
