// PanelError.tsx — the one line a panel shows when a listing could not be read, with Retry.
// The raw exception text is the tooltip only: it is not written for people.

import { Button } from "@studio/ui/button";
import { m } from "../paraglide/messages.js";

export function PanelError({
  what,
  detail,
  onRetry,
}: {
  /** What could not be read, already translated: "the resources". */
  what: string;
  /** The raw error, kept as the tooltip. */
  detail: string;
  onRetry: () => void;
}) {
  return (
    <p
      role="alert"
      title={detail}
      className="flex shrink-0 flex-wrap items-center gap-x-2 border-b px-2 py-1.5 text-error text-xs"
    >
      {m["panel.readFailed"]({ what })}
      <Button type="button" variant="link" size="xs" onClick={onRetry}>
        {m["common.retry"]()}
      </Button>
    </p>
  );
}
