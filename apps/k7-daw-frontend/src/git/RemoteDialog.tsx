// RemoteDialog.tsx — the dialog both "Connect" and "Clone" ask through: an ssh address, the device's key, the host-key flow.
// What a reader gets wrong: there is no secret to type; the device's own key signs in and only its public line is shown.

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
import { type ReactNode, useEffect, useReducer, useState } from "react";
import { m } from "../paraglide/messages.js";
import { emptyForm, formAfter, ready, request } from "./connectState";
import { explain, GitError, type Remote } from "./gitApi";
import { RemoteFields } from "./RemoteFields";

export function RemoteDialog({
  open,
  onClose,
  title,
  description,
  before,
  submit,
  step,
  canSubmit = true,
  initialUrl,
  onSubmit,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  description: string;
  /** A field above the address, as the clone dialog's workspace name. */
  before?: ReactNode;
  submit: string;
  /** The narration while the call runs. */
  step: string;
  canSubmit?: boolean;
  /** An address the dialog opens with (a workspace of another device); the person may still change it. */
  initialUrl?: string;
  /** The call; its failure is shown here, and a host-key refusal opens the trust flow. */
  onSubmit: (r: Remote) => Promise<void>;
}) {
  const [form, dispatch] = useReducer(formAfter, emptyForm);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    if (open && initialUrl) dispatch({ type: "edit", value: initialUrl });
  }, [open, initialUrl]);
  const close = () => {
    dispatch({ type: "closed" });
    setError("");
    onClose();
  };
  const ok = canSubmit && ready(form);
  const trusted = form.trust?.confirmed === true;

  const go = async () => {
    if (!ok || busy) return;
    setBusy(true);
    setError("");
    try {
      await onSubmit(request(form));
      close();
    } catch (e) {
      const said = e instanceof GitError ? e : null;
      if (said?.code === "unknown-host") {
        dispatch({
          type: "unknownHost",
          fingerprint: String(said.body.fingerprint ?? ""),
        });
      } else if (said?.code === "host-key-changed") {
        dispatch({
          type: "hostKeyChanged",
          expected: String(said.body.expected ?? ""),
          got: String(said.body.fingerprint ?? ""),
        });
      } else {
        dispatch({ type: "refused" });
        setError(explain(e));
      }
    } finally {
      setBusy(false);
    }
  };

  // Trusting the server is the answer to the call that stopped on it, so the call goes again at once.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `trusted` is the trigger; `go` of this render holds the confirmed form.
  useEffect(() => {
    if (trusted) void go();
  }, [trusted]);

  return (
    <Dialog open={open} onOpenChange={(o) => !busy && !o && close()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <form
          className="contents"
          onSubmit={(e) => {
            e.preventDefault();
            void go();
          }}
        >
          {before}
          <RemoteFields
            form={form}
            dispatch={dispatch}
            busy={busy}
            fail={setError}
            focusAddress={!before}
          />
          {error && (
            <p
              role="alert"
              className="whitespace-pre-wrap rounded-xl border border-(--k7-error-line) bg-(--k7-error-fill) px-3 py-2 text-error type-secondary"
            >
              {error}
            </p>
          )}
          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              disabled={busy}
              onClick={close}
            >
              {m["common.cancel"]()}
            </Button>
            <Button type="submit" disabled={!ok || busy}>
              {busy && <Loader2 className="mr-1 size-3 animate-spin" />}
              {busy ? step : submit}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
