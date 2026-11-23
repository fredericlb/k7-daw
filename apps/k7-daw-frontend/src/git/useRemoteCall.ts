// useRemoteCall.ts — the one call a git page makes (connect or clone) and what the host may answer: the host-key questions.
// What a reader gets wrong: a picked repository and a typed address go through the same call, so the trust flow is shared.

import { useEffect, useReducer, useState } from "react";
import { emptyForm, type FormEvent, formAfter, request } from "./connectState";
import { explain, GitError, type Remote } from "./gitApi";

export function useRemoteCall(
  onSubmit: (r: Remote) => Promise<void>,
  initialUrl = "",
) {
  const [form, dispatch] = useReducer(formAfter, emptyForm);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    if (initialUrl) dispatch({ type: "edit", value: initialUrl });
  }, [initialUrl]);

  /** Sends the form, or `picked` (a repository chosen in a picker); a host-key refusal opens the trust flow. */
  const go = async (picked?: Remote): Promise<boolean> => {
    if (busy) return false;
    setBusy(true);
    setError("");
    try {
      await onSubmit(picked ?? request(form));
      return true;
    } catch (e) {
      const said = e instanceof GitError ? e : null;
      if (picked) dispatch({ type: "edit", value: picked.url });
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
      return false;
    } finally {
      setBusy(false);
    }
  };

  const trusted = form.trust?.confirmed === true;
  // Trusting the server is the answer to the call that stopped on it, so the call goes again at once.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `trusted` is the trigger; `go` of this render holds the confirmed form.
  useEffect(() => {
    if (trusted) void go();
  }, [trusted]);

  const edit = (e: FormEvent) => dispatch(e);
  return { form, dispatch: edit, busy, error, setError, go };
}
