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

export function GitKey({ fail }: { fail: (message: string) => void }) {
  const t = useGitTransport();
  const [publicKey, setPublicKey] = useState("");
  const [synced, setSynced] = useState(false);
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
      <p className="text-muted-foreground text-xs">{m["git.keyIntro"]()}</p>
      {publicKey ? (
        <>
          <textarea
            ref={area}
            readOnly
            aria-label={m["git.publicKey"]()}
            className="min-h-16 rounded border bg-muted px-2 py-1 font-mono text-xs"
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
              <Check className="mr-1 h-3 w-3" />
            ) : (
              <Copy className="mr-1 h-3 w-3" />
            )}
            {copy === "copied" ? m["git.keyCopied"]() : m["git.keyCopy"]()}
          </Button>
          {copy === "failed" && (
            <p role="status" className="text-muted-foreground text-xs">
              {m["git.keyCopyFailed"]()}
            </p>
          )}
          <p className="text-muted-foreground text-xs">
            {synced ? m["git.keyShared"]() : m["git.keyLocal"]()}
          </p>
          <p className="text-muted-foreground text-xs">
            {m["git.keyInstall"]()}
          </p>
        </>
      ) : (
        <p className="text-muted-foreground text-xs">
          <Loader2 className="mr-1 inline h-3 w-3 animate-spin" />
          {m["git.keyLoading"]()}
        </p>
      )}
    </div>
  );
}
