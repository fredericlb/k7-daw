// MergeDialog.tsx — the Conflict screen, a full screen over the project: the list of conflicts, the open one's two sides and choice.
// The editors are read-only meanwhile (gitLock.ts); the choices are the person's alone, sent only on Finish merge.

import { showToast } from "@studio/project-shell/toast";
import { Button } from "@studio/ui/button";
import { Dialog, DialogContent, DialogTitle } from "@studio/ui/dialog";
import { ChevronLeft, ChevronRight } from "@studio/ui/icons";
import { Spinner } from "@studio/ui/spinner";
import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { m } from "../paraglide/messages.js";
import { ConflictCard } from "./ConflictCard";
import { explain, type GitTransport, git } from "./gitApi";
import { remoteName } from "./gitState";
import { MergeList } from "./MergeList";
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
  const [from, setFrom] = useState("");
  // The abort is asked inside this dialog: a popin over a modal is drawn under it and cannot be clicked.
  const [confirming, setConfirming] = useState(false);
  const contentRef = useRef<HTMLDivElement>(null);

  const load = useCallback(async () => {
    if (!t) return;
    try {
      const v = await git.merge(t, workspace);
      dispatch({ type: "load", conflicts: v.pending ? v.conflicts : [] });
      setError("");
      const g = await git.state(t, workspace).catch(() => null);
      setFrom(g?.remote ? `${remoteName(g.remote)} · ${g.branch ?? ""}` : "");
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

  const where = [workspace, from].filter(Boolean).join(" · ");
  const run = async (f: () => Promise<unknown>, merged = false) => {
    setBusy(true);
    setError("");
    try {
      await f();
      onDone();
      onClose();
      if (merged) showToast({ title: m["git.pulledMerged"](), message: where });
    } catch (e) {
      setError(explain(e));
    } finally {
      setBusy(false);
    }
  };
  const finish = () =>
    t && run(() => git.resolve(t, workspace, resolveChoices(s)), true);
  const abort = () => t && run(() => git.abort(t, workspace));

  const left = remaining(s);
  const at = current ? ordered.indexOf(current) : -1;
  const step = (by: number) => {
    const to = ordered[(at + by + ordered.length) % ordered.length];
    if (to) setOpenId(to.id);
  };
  const next = current ? nextOpen(s, current.id) : null;
  return (
    <Dialog
      open={open}
      onOpenChange={(on) => {
        if (!on && !busy && !confirming) onClose();
      }}
    >
      <DialogContent
        ref={contentRef}
        aria-describedby={undefined}
        showCloseButton={false}
        // The first conflict takes focus on opening, so a keyboard starts where the work is.
        onOpenAutoFocus={(e) => {
          const target = contentRef.current?.querySelector<HTMLElement>(
            '[role="listbox"] [tabindex="0"]',
          );
          if (target) {
            e.preventDefault();
            target.focus();
          }
        }}
        // Escape leaves the abort question before it leaves the screen, and never mid-write.
        onEscapeKeyDown={(e) => {
          if (busy) e.preventDefault();
          else if (confirming) {
            e.preventDefault();
            setConfirming(false);
          }
        }}
        className="left-0 top-0 flex h-dvh max-h-none w-screen max-w-none translate-x-0 translate-y-0 flex-col gap-0 rounded-none bg-ground p-0 text-fg-1 sm:max-w-none"
      >
        <header className="flex flex-wrap items-center gap-3 border-b border-(--k7-hairline) px-4 py-3">
          <Button variant="ghost" disabled={busy} onClick={onClose}>
            {m["git.later"]()}
          </Button>
          <div className="flex min-w-0 flex-1 flex-col">
            <DialogTitle className="type-headline">
              {m["git.mergeTitle"]()}
            </DialogTitle>
            <p className="truncate type-secondary text-fg-2">{where}</p>
          </div>
          {left > 0 && (
            <span className="type-secondary text-fg-2 tabular-nums">
              {m["git.finishLeft"]({ n: left })}
            </span>
          )}
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
            {busy && <Spinner size="xs" inherit className="mr-1" />}
            {m["git.finish"]()}
          </Button>
        </header>
        {confirming && (
          <div
            role="dialog"
            aria-label={m["git.abortTitle"]()}
            className="m-4 flex flex-col gap-2 rounded-2xl border border-(--k7-error-line) p-3"
          >
            <p className="type-label">{m["git.abortTitle"]()}</p>
            <p className="type-secondary text-fg-2">{m["git.abortBody"]()}</p>
            <div className="flex justify-end gap-2">
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
                {busy && <Spinner size="xs" inherit className="mr-1" />}
                {m["git.abort"]()}
              </Button>
            </div>
          </div>
        )}
        <div className="grid min-h-0 flex-1 grid-cols-1 gap-4 overflow-y-auto p-4 md:grid-cols-[minmax(240px,1fr)_2fr]">
          {!loaded && <Spinner size="sm" inherit />}
          {loaded && ordered.length === 0 && !error && (
            <p className="type-secondary text-fg-2">{m["git.mergeNone"]()}</p>
          )}
          {ordered.length > 0 && (
            <MergeList
              conflicts={ordered}
              chosen={s.chosen}
              currentId={current?.id}
              disabled={busy}
              onOpen={setOpenId}
              onAll={(choice) => dispatch({ type: "pickAll", choice })}
            />
          )}
          {current && (
            <div className="flex min-w-0 flex-col gap-3">
              <div className="flex items-center gap-2">
                <span className="min-w-0 flex-1 truncate type-secondary text-fg-2">
                  {current.path}
                </span>
                <span className="type-secondary text-fg-2 tabular-nums">
                  {m["git.conflictOf"]({ n: at + 1, total: ordered.length })}
                </span>
                {ordered.length > 1 && (
                  <>
                    <Button
                      size="icon"
                      variant="ghost"
                      title={m["git.previousConflict"]()}
                      aria-label={m["git.previousConflict"]()}
                      onClick={() => step(-1)}
                    >
                      <ChevronLeft className="size-4" />
                    </Button>
                    <Button
                      size="icon"
                      variant="ghost"
                      title={m["git.nextConflict"]()}
                      aria-label={m["git.nextConflict"]()}
                      onClick={() => step(1)}
                    >
                      <ChevronRight className="size-4" />
                    </Button>
                  </>
                )}
              </div>
              <ConflictCard
                conflict={current}
                chosen={s.chosen[current.id]}
                disabled={busy}
                theirsFrom={from}
                onPick={(choice) =>
                  dispatch({ type: "pick", id: current.id, choice })
                }
              />
              {!next && ordered.length > 1 && (
                <span className="type-secondary text-ok">
                  {m["git.allChosen"]()}
                </span>
              )}
              {error && (
                <p
                  role="alert"
                  className="whitespace-pre-wrap text-error text-xs"
                >
                  {error}
                </p>
              )}
            </div>
          )}
          {error && !current && (
            <p role="alert" className="whitespace-pre-wrap text-error text-xs">
              {error}
            </p>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
