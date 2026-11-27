// OtherHostForm.tsx — the Other host tab of the git pages: an ssh address, the device's key, the host-key questions, a submit.
// What a reader gets wrong: there is no secret to type; the device's own key signs in and only its public line is shown.

import { Button } from "@studio/ui/button";
import { Spinner } from "@studio/ui/spinner";
import type { ReactNode } from "react";
import { ready } from "./connectState";
import { RemoteFields } from "./RemoteFields";
import type { useRemoteCall } from "./useRemoteCall";

export function OtherHostForm({
  call,
  before,
  submit,
  step,
  canSubmit = true,
  hideKey = false,
}: {
  call: ReturnType<typeof useRemoteCall>;
  /** A field above the address, as Clone's workspace name. */
  before?: ReactNode;
  submit: string;
  /** The narration while the call runs. */
  step: string;
  canSubmit?: boolean;
  /** The address came from a picker: the key is already on the account. */
  hideKey?: boolean;
}) {
  const { form, dispatch, busy, error, setError, go } = call;
  const ok = canSubmit && ready(form) && !busy;
  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(e) => {
        e.preventDefault();
        if (ok) void go();
      }}
    >
      {before}
      <RemoteFields
        form={form}
        dispatch={dispatch}
        busy={busy}
        fail={setError}
        focusAddress={!before}
        hideKey={hideKey}
      />
      {error && (
        <p
          role="alert"
          className="whitespace-pre-wrap rounded-xl border border-(--k7-error-line) bg-(--k7-error-fill) px-3 py-2 text-error type-secondary"
        >
          {error}
        </p>
      )}
      <Button type="submit" className="self-start" disabled={!ok}>
        {busy && <Spinner size="xs" inherit className="mr-1" />}
        {busy ? step : submit}
      </Button>
    </form>
  );
}
