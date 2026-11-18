// MergeDialog.tsx — the merge screen: one card per conflict, "Finish merge" once all are chosen, "Abort merge".
// The editors are read-only meanwhile (gitLock.ts); the choices are the person's alone, sent only on Finish.

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
import { useCallback, useEffect, useReducer, useState } from "react";
import { m } from "../paraglide/messages.js";
import { ConflictCard } from "./ConflictCard";
import { explain, type GitTransport, git } from "./gitApi";
import {
  byFile,
  canFinish,
  choicesAfter,
  emptyChoices,
  remaining,
  resolveChoices,
} from "./mergeState";

export function MergeDialog({
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
  const [s, dispatch] = useReducer(choicesAfter, emptyChoices);
  const [loaded, setLoaded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  // The abort is confirmed inside this dialog: a popin over a modal is drawn under it and cannot be clicked.
  const [confirming, setConfirming] = useState(false);

  const load = useCallback(async () => {
    if (!t) return;
    try {
      const v = await git.merge(t, workspace);
      dispatch({ type: "load", conflicts: v.pending ? v.conflicts : [] });
      setError("");
    } catch (e) {
      setError(explain(e));
    } finally {
      setLoaded(true);
    }
  }, [t, workspace]);

  useEffect(() => {
    setConfirming(false);
    if (!open) return setLoaded(false);
    void load();
  }, [open, load]);

  const run = async (f: () => Promise<unknown>) => {
    setBusy(true);
    setError("");
    try {
      await f();
      onDone();
      onClose();
    } catch (e) {
      setError(explain(e));
    } finally {
      setBusy(false);
    }
  };
  const finish = () =>
    t && run(() => git.resolve(t, workspace, resolveChoices(s)));
  const abort = () => t && run(() => git.abort(t, workspace));

  const left = remaining(s);
  return (
    <Dialog open={open} onOpenChange={(o) => !busy && !o && onClose()}>
      <DialogContent className="max-h-[85vh] max-w-2xl overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{m["git.mergeTitle"]()}</DialogTitle>
          <DialogDescription>{m["git.mergeBody"]()}</DialogDescription>
        </DialogHeader>
        {!loaded && <Loader2 className="h-4 w-4 animate-spin" />}
        {loaded && s.conflicts.length === 0 && !error && (
          <p className="text-muted-foreground text-xs">
            {m["git.mergeNone"]()}
          </p>
        )}
        {byFile(s.conflicts).map((g) => (
          <section key={g.path} className="flex flex-col gap-2">
            <h3 className="truncate font-mono text-xs">{g.path}</h3>
            <ul className="flex flex-col gap-2">
              {g.items.map((c) => (
                <ConflictCard
                  key={c.id}
                  conflict={c}
                  chosen={s.chosen[c.id]}
                  disabled={busy}
                  onPick={(choice) =>
                    dispatch({ type: "pick", id: c.id, choice })
                  }
                />
              ))}
            </ul>
          </section>
        ))}
        {s.conflicts.length > 1 && (
          <div className="flex gap-2">
            {(["mine", "theirs"] as const).map((side) => (
              <Button
                key={side}
                size="xs"
                variant="ghost"
                disabled={busy}
                onClick={() => dispatch({ type: "pickAll", choice: side })}
              >
                {m[side === "mine" ? "git.allMine" : "git.allTheirs"]()}
              </Button>
            ))}
          </div>
        )}
        {error && (
          <p
            role="alert"
            className="whitespace-pre-wrap text-destructive text-xs"
          >
            {error}
          </p>
        )}
        {confirming ? (
          <div
            role="alertdialog"
            aria-label={m["git.abortTitle"]()}
            className="flex flex-col gap-2 rounded-md border border-destructive/40 p-3"
          >
            <p className="font-semibold text-sm">{m["git.abortTitle"]()}</p>
            <p className="text-muted-foreground text-xs">
              {m["git.abortBody"]()}
            </p>
            <DialogFooter>
              <Button
                variant="ghost"
                disabled={busy}
                onClick={() => setConfirming(false)}
              >
                {m["git.abortKeep"]()}
              </Button>
              <Button
                variant="destructive"
                disabled={busy || !t}
                onClick={() => void abort()}
              >
                {busy && <Loader2 className="mr-1 h-3 w-3 animate-spin" />}
                {m["git.abort"]()}
              </Button>
            </DialogFooter>
          </div>
        ) : (
          <DialogFooter>
            <Button variant="ghost" disabled={busy} onClick={onClose}>
              {m["git.later"]()}
            </Button>
            <Button
              variant="outline"
              disabled={busy || !t}
              onClick={() => setConfirming(true)}
            >
              {m["git.abort"]()}
            </Button>
            <Button
              disabled={busy || !canFinish(s)}
              onClick={() => void finish()}
            >
              {busy && <Loader2 className="mr-1 h-3 w-3 animate-spin" />}
              {left > 0 ? m["git.finishLeft"]({ n: left }) : m["git.finish"]()}
            </Button>
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  );
}
