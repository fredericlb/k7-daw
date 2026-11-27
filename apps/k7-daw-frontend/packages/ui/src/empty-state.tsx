import type * as React from "react";
import { cn } from "./lib/utils";

// Drawn only when there is nothing; `compact` is the 64 px glyph and 18 px title of an empty panel.
export function EmptyState({
  glyph,
  title,
  children,
  actions,
  compact,
  className,
}: {
  glyph: React.ReactNode;
  title: React.ReactNode;
  children?: React.ReactNode;
  actions?: React.ReactNode;
  compact?: boolean;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "flex flex-col items-center gap-3 rounded-[24px] border-[1.5px] border-dashed border-(--k7-hairline-strong) px-6 py-8 text-center",
        className,
      )}
    >
      <span
        className={cn(
          "grid place-items-center rounded-full bg-(--k7-glyph-tile) text-primary-text",
          compact ? "size-16 [&_svg]:size-7" : "size-[76px] [&_svg]:size-8",
        )}
      >
        {glyph}
      </span>
      <h3
        className={cn(
          "font-bold text-fg-1",
          compact ? "text-[18px]" : "type-sheet-title",
        )}
      >
        {title}
      </h3>
      {children ? (
        <p className="type-secondary max-w-[32ch] text-fg-2">{children}</p>
      ) : null}
      {actions ? (
        <div className="mt-1 flex flex-wrap justify-center gap-2">
          {actions}
        </div>
      ) : null}
    </div>
  );
}
