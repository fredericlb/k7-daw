// SharesEndNotice.tsx — the home page's one line saying sharing projects by link ends, dismissable per browser.
// The date lives in sharesEnd.ts, so both languages read the same one.

import { Button } from "@studio/ui/button";
import { Info, X } from "@studio/ui/icons";
import { useState } from "react";
import { m } from "../paraglide/messages.js";
import { dismissSharesEnd, SHARES_END, sharesEndDismissed } from "./sharesEnd";

export function SharesEndNotice() {
  const [hidden, setHidden] = useState(sharesEndDismissed);
  if (hidden) return null;
  return (
    <div
      role="note"
      className="flex items-center gap-2 border-b bg-muted/40 px-6 py-3 text-xs"
    >
      <Info className="h-4 w-4 shrink-0 text-muted-foreground" />
      <span className="flex-1">
        {m["home.sharesEnd"]({ date: SHARES_END })}
      </span>
      <Button
        variant="ghost"
        size="icon"
        title={m["common.dismiss"]()}
        aria-label={m["common.dismiss"]()}
        onClick={() => {
          dismissSharesEnd();
          setHidden(true);
        }}
      >
        <X className="h-3.5 w-3.5" />
      </Button>
    </div>
  );
}
