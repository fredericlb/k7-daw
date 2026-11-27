// EngineSyncFailedDialog.tsx — the blocking dialog when the project's files did not all reach the
// engine's machine. FATAL by design: the project is held back rather than played with silent
// clips, and the only way out is to reload it, which copies again. Not dismissable.
// See apps/k7-daw-backend/docs/file-engine.md.

import { Button } from "@studio/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@studio/ui/dialog";
import { Spinner } from "@studio/ui/spinner";
import type { EngineSyncState } from "@studio/web-protocol";
import { useEffect, useState } from "react";
import { m } from "../paraglide/messages.js";

/** How many failed files are listed before the rest is summed up. */
const LISTED = 8;
/** Two polls: by then the retry is either copying, and the dialog gone, or has failed again. */
const RETRY_SETTLE_MS = 4000;

/** Whether `s` is a copy still running, as opposed to the report of one that went wrong. */
export const syncRunning = (s: EngineSyncState | null | undefined): boolean =>
  !!s && s.error === undefined && s.failed === undefined;

export function EngineSyncFailedDialog({
  sync,
  onReload,
}: {
  sync: EngineSyncState | null; // a failed copy, or null when there is none
  onReload: () => Promise<void>;
}) {
  const [reloading, setReloading] = useState(false);
  const open = !!sync;
  // The dialog closing is the retry having started; a retry that fails before the poll sees it is the timeout's.
  useEffect(() => {
    if (!open) setReloading(false);
  }, [open]);
  const failures = sync?.failures ?? [];
  return (
    <Dialog open={open}>
      <DialogContent
        className="max-w-lg"
        showCloseButton={false}
        onEscapeKeyDown={(e) => e.preventDefault()}
        onPointerDownOutside={(e) => e.preventDefault()}
        onInteractOutside={(e) => e.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle>{m["engineSync.failedTitle"]()}</DialogTitle>
          <DialogDescription>
            {sync?.error
              ? m["engineSync.errorBody"]()
              : m["engineSync.failedBody"]()}
          </DialogDescription>
        </DialogHeader>
        {(sync?.error || failures.length > 0) && (
          <div className="max-h-60 overflow-y-auto rounded bg-row px-2 py-1.5 font-mono text-xs text-red-400">
            {sync?.error && <div>{sync.error}</div>}
            {failures.slice(0, LISTED).map((line) => (
              <div key={line}>{line}</div>
            ))}
            {failures.length > LISTED && (
              <div>… +{failures.length - LISTED}</div>
            )}
          </div>
        )}
        <DialogFooter>
          <Button
            disabled={reloading}
            onClick={() => {
              setReloading(true);
              onReload()
                .then(() =>
                  setTimeout(() => setReloading(false), RETRY_SETTLE_MS),
                )
                .catch(() => setReloading(false));
            }}
          >
            {reloading && <Spinner size="sm" inherit className="mr-1" />}
            {reloading ? m["engineSync.reloading"]() : m["engineSync.reload"]()}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
