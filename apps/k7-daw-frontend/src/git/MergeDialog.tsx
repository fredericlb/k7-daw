// MergeDialog.tsx — the Conflict screen: a list of conflicts, the open one's choices, Finish once every one has a choice.
// The editors are read-only meanwhile (gitLock.ts); the choices are the person's alone, sent only on Finish.

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
import { cn } from "@studio/ui/lib/utils";
import { useCallback, useEffect, useReducer, useState } from "react";
import { m } from "../paraglide/messages.js";
import { ConflictCard, choiceLabel, whereOf } from "./ConflictCard";
import { explain, type GitTransport, git } from "./gitApi";
import {
  byFile,
  canFinish,
  choicesAfter,
  emptyChoices,
  nextOpen,
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
  const [openId, setOpenId] = useState<string | null>(null);
  // The abort is asked inside this dialog: a popin over a modal is drawn under it and cannot be clicked.
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

  const ordered = byFile(s.conflicts).flatMap((g) => g.items);
  const current =
    ordered.find((c) => c.id === openId) ??
    ordered.find((c) => !s.chosen[c.id]) ??
    ordered[0];

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
  const next = current ? nextOpen(s, current.id) : null;
  return (
    <Dialog open={open} onOpenChange={(o) => !busy && !o && onClose()}>
      <DialogContent className="max-h-[85vh] max-w-2xl overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{m["git.conflictTitle"]()}</DialogTitle>
          <DialogDescription>{m["git.mergeBody"]()}</DialogDescription>
        </DialogHeader>
        {!loaded && <Loader2 className="size-4 animate-spin" />}
        {loaded && ordered.length === 0 && !error && (
          <p className="type-secondary text-fg-2">{m["git.mergeNone"]()}</p>
        )}
        {ordered.length > 0 && (
          <div
            role="listbox"
            aria-label={m["git.conflictList"]()}
            className="flex max-h-44 flex-col gap-1 overflow-y-auto"
          >
            {ordered.map((c) => (
              <button
                key={c.id}
                type="button"
                role="option"
                aria-selected={c.id === current?.id}
                onClick={() => setOpenId(c.id)}
                className={cn(
                  "flex cursor-pointer items-center gap-2 rounded-xl px-3 py-2 text-left outline-none focus-visible:shadow-(--k7-focus-ring)",
                  c.id === current?.id ? "bg-on-fill" : "hover:bg-on-fill",
                )}
              >
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate type-mono-value">{whereOf(c)}</span>
                  <span className="truncate type-secondary text-fg-2">
                    {c.path}
                  </span>
                </span>
                <Chip
                  variant="status"
                  tone={s.chosen[c.id] ? "renamed" : "choose"}
                >
                  {s.chosen[c.id]
                    ? choiceLabel(s.chosen[c.id])
                    : m["git.toChoose"]()}
                </Chip>
              </button>
            ))}
          </div>
        )}
        {current && (
          <ConflictCard
            conflict={current}
            chosen={s.chosen[current.id]}
            disabled={busy}
            onPick={(choice) =>
              dispatch({ type: "pick", id: current.id, choice })
            }
          />
        )}
        {ordered.length > 1 && (
          <div className="flex flex-wrap items-center gap-2">
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
            {next ? (
              <Button
                size="xs"
                variant="secondary"
                className="ml-auto"
                onClick={() => setOpenId(next)}
              >
                {m["git.nextConflict"]()}
              </Button>
            ) : (
              <span className="ml-auto type-secondary text-ok">
                {m["git.allChosen"]()}
              </span>
            )}
          </div>
        )}
        {error && (
          <p role="alert" className="whitespace-pre-wrap text-error text-xs">
            {error}
          </p>
        )}
        {confirming ? (
          <div
            role="dialog"
            aria-label={m["git.abortTitle"]()}
            className="flex flex-col gap-2 rounded-2xl border border-(--k7-error-line) p-3"
          >
            <p className="type-label">{m["git.abortTitle"]()}</p>
            <p className="type-secondary text-fg-2">{m["git.abortBody"]()}</p>
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
                {busy && <Loader2 className="mr-1 size-3 animate-spin" />}
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
              {busy && <Loader2 className="mr-1 size-3 animate-spin" />}
              {left > 0 ? m["git.finishLeft"]({ n: left }) : m["git.finish"]()}
            </Button>
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  );
}
