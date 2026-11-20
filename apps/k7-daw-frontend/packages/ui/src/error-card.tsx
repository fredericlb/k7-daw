import type * as React from "react";
import { TriangleAlert } from "./icons";
import { cn } from "./lib/utils";

// A failure that stays until fixed: error border and tint, the title, and the cause in one line.
export function ErrorCard({
  title,
  children,
  className,
}: {
  title: React.ReactNode;
  children?: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      role="alert"
      className={cn(
        "flex items-start gap-3 rounded-(--k7-r-rows) border border-(--k7-error-line) bg-(--k7-error-fill) px-3.5 py-3",
        className,
      )}
    >
      <TriangleAlert
        className="mt-0.5 size-5 shrink-0 text-error"
        aria-hidden
      />
      <div className="min-w-0">
        <p className="type-label text-foreground">{title}</p>
        {children ? (
          <p className="type-secondary text-fg-2">{children}</p>
        ) : null}
      </div>
    </div>
  );
}
