// empty-row.tsx — the one line an overlay's list shows when it has nothing: "no match", or "…" while it is still being read.

import type * as React from "react";
import { cn } from "./lib/utils";
import { Spinner } from "./spinner";

export function EmptyRow({
  children,
  pending,
  className,
}: {
  children: React.ReactNode;
  /** The list is still being read: a spinner in place of the sentence, which would claim there is nothing. */
  pending?: boolean;
  className?: string;
}) {
  return (
    <div
      role="status"
      className={cn(
        "type-secondary flex min-h-11 items-center gap-2 px-3 py-3 text-fg-3 desk:min-h-8",
        className,
      )}
    >
      {pending ? <Spinner size="xs" inherit /> : children}
    </div>
  );
}
