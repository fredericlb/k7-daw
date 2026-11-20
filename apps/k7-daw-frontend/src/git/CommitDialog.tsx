// CommitDialog.tsx — Commit: the changed files with their sizes (large ones marked), a message, the whole tree.
// What a reader gets wrong: there is no per-file choice, and a file over the host's limit is left out and named.

import { Button } from "@studio/ui/button";
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
import {
  commitView,
  formatSize,
  identityReady,
  identityToSave,
} from "./commitState";
import { explain, type GitTransport, git, type Identity } from "./gitApi";
import type { StatusEntry } from "./gitTypes";
import { saveIdentity } from "./identity";

const CHANGE = {
  added: "git.changeAdded",
  modified: "git.changeModified",
  removed: "git.changeRemoved",
  renamed: "git.changeRenamed",
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
  const [message, setMessage] = useState("");
  const [who, setWho] = useState<Identity | null>(null);
  const [prefilled, setPrefilled] = useState<Identity | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!open || !t) return;
    let live = true;
    setEntries(null);
    setError("");
    git
      .status(t, workspace)
      .then((r) => live && setEntries(r.entries))
      .catch((e) => live && setError(explain(e)));
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
  const signed = identityReady(who);
  const ok = view.block === null && signed && !busy;

  const go = async () => {
    if (!t || !ok || !who) return;
    setBusy(true);
    setError("");
    try {
      const save = identityToSave(prefilled, who);
      if (save) await saveIdentity(t, save);
      await git.commit(
        t,
        workspace,
        message.trim(),
        view.leftOut.map((e) => e.path),
      );
      setMessage("");
      onDone();
      onClose();
    } catch (e) {
      setError(explain(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(o) => !busy && !o && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{m["git.commitTitle"]()}</DialogTitle>
          <DialogDescription>{m["git.commitBody"]()}</DialogDescription>
        </DialogHeader>
        <ul
          aria-label={m["git.files"]()}
          className="max-h-56 overflow-y-auto rounded border text-xs"
        >
          {entries === null && !error && (
            <li className="px-3 py-2 text-muted-foreground">
              <Loader2 className="mr-1 inline h-3 w-3 animate-spin" />
              {m["git.reading"]()}
            </li>
          )}
          {entries?.length === 0 && (
            <li className="px-3 py-2 text-muted-foreground">
              {m["git.noChanges"]()}
            </li>
          )}
          {view.rows.map((e) => (
            <li
              key={e.path}
              className="flex items-center gap-2 border-b px-3 py-1 last:border-b-0"
            >
              <span className="w-16 shrink-0 text-muted-foreground">
                {m[CHANGE[e.change]]()}
              </span>
              <span
                className="min-w-0 flex-1 truncate font-mono"
                title={e.from ? `${e.from} → ${e.path}` : e.path}
              >
                {e.path}
              </span>
              {e.lfs && (
                <span
                  className="rounded bg-muted px-1 text-[10px]"
                  title={m["git.lfsHint"]()}
                >
                  {m["git.lfs"]()}
                </span>
              )}
              <span className="w-16 shrink-0 text-right tabular-nums text-muted-foreground">
                {formatSize(e.size)}
              </span>
            </li>
          ))}
        </ul>
        {view.leftOut.map((e) => (
          <p key={e.path} className="text-muted-foreground text-xs">
            {m["git.leftOut"]({ name: e.path, size: formatSize(e.size) })}
          </p>
        ))}
        {who && !signed && (
          <div className="flex flex-col gap-2">
            <p className="text-muted-foreground text-xs">
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
        <textarea
          aria-label={m["git.message"]()}
          placeholder={m["git.message"]()}
          className="min-h-20 rounded border bg-background px-2 py-1 text-sm"
          value={message}
          disabled={busy}
          onChange={(e) => setMessage(e.target.value)}
        />
        {error && (
          <p
            role="alert"
            className="whitespace-pre-wrap text-destructive text-xs"
          >
            {error}
          </p>
        )}
        <DialogFooter>
          <Button variant="ghost" disabled={busy} onClick={onClose}>
            {m["common.cancel"]()}
          </Button>
          <Button disabled={!ok} onClick={() => void go()}>
            {busy && <Loader2 className="mr-1 h-3 w-3 animate-spin" />}
            {m["git.commit"]()}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
