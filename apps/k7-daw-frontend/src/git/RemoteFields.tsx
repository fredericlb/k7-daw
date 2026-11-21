// RemoteFields.tsx — the fields of Connect and Clone: the ssh address, the device's public key, and the host-key warnings.
// What a reader gets wrong: a first-contact fingerprint is shown and confirmed here; no https field exists, k7 is ssh only.

import { Button } from "@studio/ui/button";
import { TriangleAlert } from "@studio/ui/icons";
import { Input } from "@studio/ui/input";
import { useId, useState } from "react";
import { m } from "../paraglide/messages.js";
import {
  type FormEvent,
  type FormFields,
  isWebAddress,
  sshHost,
} from "./connectState";
import { useGitTransport } from "./GitContext";
import { GitKey } from "./GitKey";
import { explain, git } from "./gitApi";

export function RemoteFields({
  form,
  dispatch,
  busy,
  fail,
  focusAddress = true,
}: {
  form: FormFields;
  dispatch: (e: FormEvent) => void;
  busy: boolean;
  fail: (message: string) => void;
  /** False when a field sits above the address: that one is first, and takes the focus. */
  focusAddress?: boolean;
}) {
  const t = useGitTransport();
  const urlId = useId();
  const [forgetting, setForgetting] = useState(false);
  const host = sshHost(form.url)?.host ?? form.url.trim();
  const forget = async () => {
    const target = sshHost(form.url);
    if (!t || !target) return;
    setForgetting(true);
    try {
      await git.forgetHost(t, target);
      dispatch({ type: "hostForgotten" });
    } catch (e) {
      fail(explain(e));
    } finally {
      setForgetting(false);
    }
  };
  return (
    <>
      <div className="flex flex-col gap-1.5">
        <label htmlFor={urlId} className="type-label">
          {m["git.url"]()}
        </label>
        <Input
          id={urlId}
          autoFocus={focusAddress}
          placeholder={m["git.urlPlaceholder"]()}
          autoComplete="off"
          value={form.url}
          disabled={busy}
          onChange={(e) => dispatch({ type: "edit", value: e.target.value })}
        />
      </div>
      {isWebAddress(form.url) && (
        <p role="alert" className="text-error type-secondary">
          {m["git.sshOnly"]()}
        </p>
      )}
      {!form.trust && !form.changed && <GitKey fail={fail} />}
      {form.trust && (
        <div
          role="alert"
          className="flex flex-col gap-2 rounded-2xl border border-(--k7-hairline-strong) bg-(--k7-row) px-3 py-2 type-secondary"
        >
          <p>{m["git.hostUnknown"]({ host })}</p>
          <code className="break-all type-mono-value">
            {form.trust.fingerprint}
          </code>
          <p className="text-fg-2">{m["git.hostUnknownNote"]()}</p>
          <Button
            size="sm"
            variant={form.trust.confirmed ? "secondary" : "default"}
            className="self-start"
            disabled={busy || form.trust.confirmed}
            onClick={() => dispatch({ type: "confirm" })}
          >
            {form.trust.confirmed
              ? m["git.hostTrusted"]()
              : m["git.hostTrust"]()}
          </Button>
        </div>
      )}
      {form.changed && (
        <div
          role="alert"
          className="flex flex-col gap-1 rounded-2xl border border-(--k7-error-line) bg-(--k7-error-fill) px-3 py-2 text-error type-secondary"
        >
          <p className="font-medium">
            <TriangleAlert className="mr-1 inline size-3" />
            {m["git.hostChanged"]({ host })}
          </p>
          <p>{m["git.hostChangedTrusted"]()}</p>
          <code className="break-all type-mono-value">
            {form.changed.expected}
          </code>
          <p>{m["git.hostChangedShown"]()}</p>
          <code className="break-all type-mono-value">{form.changed.got}</code>
          <p>{m["git.hostChangedNote"]()}</p>
          <Button
            size="sm"
            variant="outline"
            className="self-start"
            disabled={busy || forgetting || !t}
            onClick={() => void forget()}
          >
            {m["git.hostForget"]()}
          </Button>
        </div>
      )}
    </>
  );
}
