// HistoryRollback.tsx — a journal row's way back to its state: a button, then a bubble that says how many edits move.
// The one thing a reader would get wrong: going back undoes LATER edits one by one, each still in the list to redo.

import { Button } from "@studio/ui/button";
import { RotateCcw } from "@studio/ui/icons";
import { Popover, PopoverContent, PopoverTrigger } from "@studio/ui/popover";
import { useState } from "react";
import { m } from "../paraglide/messages.js";

export function RollbackButton({
  redo,
  steps,
  onConfirm,
}: {
  redo: boolean;
  steps: number;
  onConfirm: () => Promise<void>;
}) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const go = async () => {
    setBusy(true);
    try {
      await onConfirm();
    } finally {
      setBusy(false);
      setOpen(false);
    }
  };
  return (
    <Popover open={open} onOpenChange={(o) => !busy && setOpen(o)}>
      <PopoverTrigger asChild>
        <Button
          variant="ghost"
          size="round"
          className="shrink-0"
          title={m["history.rollback"]()}
          aria-label={m["history.rollback"]()}
        >
          <RotateCcw className="size-4" />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="flex flex-col gap-3">
        <p className="type-secondary text-fg-1">
          {redo
            ? m["history.rollbackRedo"]({ count: steps })
            : m["history.rollbackUndo"]({ count: steps })}
        </p>
        <div className="flex justify-end gap-2">
          <Button
            size="sm"
            variant="ghost"
            disabled={busy}
            onClick={() => setOpen(false)}
          >
            {m["common.cancel"]()}
          </Button>
          <Button size="sm" disabled={busy} onClick={() => void go()}>
            {m["history.rollbackConfirm"]()}
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
