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
import {
  emptyForm,
  type FormEvent,
  formAfter,
  ready,
  request,
} from "./connectState";
import { useGitTransport } from "./GitContext";
import type { PickedRepo } from "./GitHubPicker";
import { GitHubWays } from "./GitHubWays";
import { explain, GitError, type Remote } from "./gitApi";
import { canPick } from "./githubState";
import { RemoteFields } from "./RemoteFields";
import { useGithub } from "./useGithub";

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
  createName,
  onRepoPicked,
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
  /** The workspace's name, which "New repository" takes on GitHub; absent where no repository can be made. */
  createName?: string;
  /** Present where picking a GitHub repository only fills the form (Clone), not sends it (Connect). */
  onRepoPicked?: (r: PickedRepo) => void;
  /** The call; its failure is shown here, and a host-key refusal opens the trust flow. */
  onSubmit: (r: Remote) => Promise<void>;
}) {
  const [form, dispatch] = useReducer(formAfter, emptyForm);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const t = useGitTransport();
  const github = useGithub(open ? t : null);
  // The person's explicit way ("Another host…"); until then GitHub's picker leads for a person signed in and nothing was typed.
  const [chosen, setChosen] = useState<"other" | null>(null);
  const [fromGithub, setFromGithub] = useState(false);
  const offered = canPick(github.status) && t !== null;
  const ways = offered && !chosen && !initialUrl;
  // Typing an address is choosing the other way, whenever the status arrived.
  const edit = (e: FormEvent) => {
    if (e.type === "edit") setChosen("other");
    dispatch(e);
  };
  useEffect(() => {
    if (open && initialUrl) dispatch({ type: "edit", value: initialUrl });
  }, [open, initialUrl]);
  const close = () => {
    dispatch({ type: "closed" });
    setChosen(null);
    setFromGithub(false);
    setError("");
    onClose();
  };
  const ok = canSubmit && ready(form);
  const trusted = form.trust?.confirmed === true;

  const go = async (picked?: Remote) => {
    if (!picked && (!ok || busy)) return;
    setBusy(true);
    setError("");
    try {
      await onSubmit(picked ?? request(form));
      close();
    } catch (e) {
      const said = e instanceof GitError ? e : null;
      if (picked) setChosen("other");
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

  const pick = (r: PickedRepo) => {
    dispatch({ type: "edit", value: r.sshUrl });
    if (onRepoPicked) {
      onRepoPicked(r);
      setFromGithub(true);
      setChosen("other");
    } else void go({ url: r.sshUrl });
  };

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
          {ways && t && github.status ? (
            <GitHubWays
              t={t}
              status={github.status}
              refresh={github.refresh}
              createName={createName}
              disabled={busy}
              onPick={pick}
              onOther={() => setChosen("other")}
            />
          ) : null}
          {ways && busy && (
            <p role="status" className="text-fg-2 type-secondary">
              <Loader2 className="mr-1 inline size-3 animate-spin" />
              {step}
            </p>
          )}
          {ways ? null : (
            <>
              {before}
              <RemoteFields
                form={form}
                dispatch={edit}
                busy={busy}
                fail={setError}
                focusAddress={!before}
                hideKey={fromGithub}
              />
            </>
          )}
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
            {!ways && (
              <Button type="submit" disabled={!ok || busy}>
                {busy && <Loader2 className="mr-1 size-3 animate-spin" />}
                {busy ? step : submit}
              </Button>
            )}
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
