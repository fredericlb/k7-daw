// GitKey.tsx — the device's own public SSH key, shown read-only with a Copy button, to add to the git host.
// What a reader gets wrong: the key is made by the device on first need and is the same for every workspace; the page
// only ever holds its public line.

import { Button } from "@studio/ui/button";
import { Check, Copy, Loader2 } from "@studio/ui/icons";
import { useEffect, useRef, useState } from "react";
import { m } from "../paraglide/messages.js";
import { copyText } from "./copyText";
import { useGitTransport } from "./GitContext";
import { explain, git } from "./gitApi";
import { OldKey } from "./OldKey";

export function GitKey({ fail }: { fail: (message: string) => void }) {
  const t = useGitTransport();
  const [publicKey, setPublicKey] = useState("");
  const [synced, setSynced] = useState(false);
  const [oldHosts, setOldHosts] = useState<string[] | null>(null);
  const [copy, setCopy] = useState<"idle" | "copied" | "failed">("idle");
  const area = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    if (!t) return;
    let live = true;
    git
      .key(t)
      .then((k) => {
        if (!live) return;
        setPublicKey(k.publicKey);
        setSynced(k.synced === true);
        setOldHosts(k.oldKey ? k.oldKey.hosts : null);
      })
      .catch((e) => live && fail(explain(e)));
    return () => {
      live = false;
    };
  }, [t, fail]);
  const copyKey = async () => {
    setCopy((await copyText(publicKey, area.current)) ? "copied" : "failed");
  };
  return (
    <div className="flex flex-col gap-2">
      <p className="text-fg-2 type-secondary">{m["git.keyIntro"]()}</p>
      {publicKey ? (
        <>
          <textarea
            ref={area}
            readOnly
            aria-label={m["git.publicKey"]()}
            className="min-h-16 rounded-xl border border-(--k7-hairline-strong) bg-(--k7-row) px-3 py-2 type-mono-value"
            value={publicKey}
            onFocus={(e) => e.currentTarget.select()}
          />
          <Button
            size="sm"
            variant="secondary"
            className="self-start"
            onClick={() => void copyKey()}
          >
            {copy === "copied" ? (
              <Check className="mr-1 size-3" />
            ) : (
              <Copy className="mr-1 size-3" />
            )}
            {copy === "copied" ? m["git.keyCopied"]() : m["git.keyCopy"]()}
          </Button>
          {copy === "failed" && (
            <p role="status" className="text-fg-2 type-secondary">
              {m["git.keyCopyFailed"]()}
            </p>
          )}
          <p className="text-fg-2 type-secondary">
            {synced ? m["git.keyShared"]() : m["git.keyLocal"]()}
          </p>
          <p className="text-fg-2 type-secondary">{m["git.keyInstall"]()}</p>
          {oldHosts && (
            <OldKey
              hosts={oldHosts}
              fail={fail}
              forgotten={() => setOldHosts(null)}
            />
          )}
        </>
      ) : (
        <p className="text-fg-2 type-secondary">
          <Loader2 className="mr-1 inline size-3 animate-spin" />
          {m["git.keyLoading"]()}
        </p>
      )}
    </div>
  );
}
