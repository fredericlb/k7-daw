// DeleteProject.tsx — the confirmation behind the row's ⋮ menu. Naming the project IN the question
// is what makes the confirmation useful: "are you sure?" does not say of what.
// See ../../docs/projects.md.

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
import { useState } from "react";
import { m } from "../paraglide/messages.js";

export function DeleteProject({
  open,
  onClose,
  /** The project's path in the workspace — what is deleted, and what the question names. */
  path,
  label,
  onDelete,
}: {
  /** Driven from the row's menu, which is what registers the gesture — see ProjectMenu.tsx. */
  open: boolean;
  onClose: () => void;
  path: string;
  label: string;
  onDelete: (onStep: (s: string) => void) => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const [step, setStep] = useState("");
  const [error, setError] = useState("");

  const submit = () => {
    setBusy(true);
    setError("");
    void onDelete(setStep)
      .then(onClose)
      .catch((e: Error) => setError(e.message))
      .finally(() => {
        setBusy(false);
        setStep("");
      });
  };

  return (
    <Dialog open={open} onOpenChange={(o) => !busy && !o && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{m["project.deleteTitle"]({ label })}</DialogTitle>
          <DialogDescription>{m["project.deleteBody"]()}</DialogDescription>
        </DialogHeader>

        <p className="rounded-xl bg-(--k7-input-fill) px-3 py-2 type-mono-value">
          {path}
        </p>
        {/* Where the contents go, said without overselling it: the recycle bin keeps them, which is not an undo button. */}
        <p className="type-secondary text-fg-2">{m["project.deleteKept"]()}</p>

        {step && (
          <p className="type-secondary text-fg-2">
            <Loader2 className="mr-1 inline h-3 w-3 animate-spin" />
            {step}
          </p>
        )}
        {error && (
          <p className="rounded-xl border border-(--k7-error-line) bg-(--k7-error-fill) px-3 py-2 type-secondary text-error">
            {error}
          </p>
        )}

        <DialogFooter>
          <Button variant="glass" disabled={busy} onClick={onClose}>
            {m["common.cancel"]()}
          </Button>
          <Button variant="destructive" disabled={busy} onClick={submit}>
            {busy && <Loader2 className="mr-1 size-5 animate-spin" />}
            {m["project.delete"]()}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
