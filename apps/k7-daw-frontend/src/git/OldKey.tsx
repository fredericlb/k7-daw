// OldKey.tsx — a Mac's own key, kept beside the shared one: the hosts that still take it, and a way to forget it.
// What a reader gets wrong: forgetting deletes the Mac's old key only; the shared key the box above shows stays.

import { Button } from "@studio/ui/button";
import { useState } from "react";
import { m } from "../paraglide/messages.js";
import { useGitTransport } from "./GitContext";
import { explain, git } from "./gitApi";

export function OldKey({
  hosts,
  fail,
  forgotten,
}: {
  hosts: string[];
  fail: (message: string) => void;
  forgotten: () => void;
}) {
  const t = useGitTransport();
  const [asking, setAsking] = useState(false);
  const [busy, setBusy] = useState(false);
  const forget = async () => {
    if (!t) return;
    setBusy(true);
    try {
      await git.forgetOldKey(t);
      forgotten();
    } catch (e) {
      fail(explain(e));
    } finally {
      setBusy(false);
      setAsking(false);
    }
  };
  return (
    <div className="flex flex-col gap-2">
      <p className="text-fg-2 type-secondary">
        {hosts.length > 0
          ? m["git.oldKeyHosts"]({ hosts: hosts.join(", ") })
          : m["git.oldKeyKept"]()}
      </p>
      {asking ? (
        <div role="alert" className="flex flex-col gap-2">
          <p className="text-fg-2 type-secondary">
            {m["git.oldKeyForgetConfirm"]()}
          </p>
          <div className="flex gap-2">
            <Button
              size="sm"
              variant="destructive"
              disabled={busy}
              onClick={() => void forget()}
            >
              {m["git.oldKeyForgetYes"]()}
            </Button>
            <Button
              size="sm"
              variant="secondary"
              disabled={busy}
              onClick={() => setAsking(false)}
            >
              {m["common.cancel"]()}
            </Button>
          </div>
        </div>
      ) : (
        <Button
          size="sm"
          variant="secondary"
          className="self-start"
          onClick={() => setAsking(true)}
        >
          {m["git.oldKeyForget"]()}
        </Button>
      )}
    </div>
  );
}
