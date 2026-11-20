// side-panel.tsx — the right panel of Pad and Desk (SidePanel) and its phone twin, the full-screen sheet.
// A panel floats OVER the panes and never pushes them; the host positions the parent (it is `absolute`).

import type * as React from "react";
import { Button } from "./button";
import { X } from "./icons";
import { cn } from "./lib/utils";

interface PanelProps {
  title: string;
  /** What the panel is, under the title: an editor's "audio · bar 2 · 0:00.50 → 0:01.75". */
  subtitle?: React.ReactNode;
  /** A 20 px glyph in accent-text before the title. */
  icon?: React.ReactNode;
  /** Buttons between the title and Close. */
  actions?: React.ReactNode;
  onClose: () => void;
  closeLabel: string;
  children: React.ReactNode;
  className?: string;
}

function Heading({
  title,
  subtitle,
  icon,
  actions,
  onClose,
  closeLabel,
  sheet,
}: Omit<PanelProps, "children" | "className"> & { sheet?: boolean }) {
  return (
    <div className="flex min-h-11 shrink-0 items-center gap-2.5">
      {icon ? (
        <span className="grid shrink-0 place-items-center text-primary-text [&_svg]:size-5">
          {icon}
        </span>
      ) : null}
      <div className="min-w-0 flex-1">
        <h2
          className={cn(
            "truncate text-foreground",
            sheet ? "type-sheet-title" : "type-headline",
          )}
        >
          {title}
        </h2>
        {subtitle ? (
          <p className="type-status truncate text-fg-2">{subtitle}</p>
        ) : null}
      </div>
      {actions}
      <Button
        variant={sheet ? "secondary" : "ghost"}
        size="round"
        aria-label={closeLabel}
        title={closeLabel}
        onClick={onClose}
        className={
          sheet ? "border-0 bg-none bg-white/10 shadow-none" : undefined
        }
      >
        <X className="size-5" strokeWidth={2.2} />
      </Button>
    </div>
  );
}

/** The 400 px floating panel: right 12, under the header, clear of the bar. */
export function SidePanel({ children, className, ...head }: PanelProps) {
  return (
    <aside
      aria-label={head.title}
      className={cn(
        "panel absolute right-(--k7-panel-right) bottom-(--k7-panel-bottom) top-[calc(var(--k7-header-h)+var(--k7-panel-gap-top))] z-(--k7-z-panel) flex w-(--k7-panel-w) max-w-[calc(100%-24px)] flex-col gap-2.5 rounded-(--k7-r-panel) p-(--k7-panel-padding) animate-[k7-panel-in_var(--k7-dur-panel)_var(--k7-ease-spring)]",
        className,
      )}
    >
      <Heading {...head} />
      <div className="min-h-0 flex-1 overflow-y-auto pr-2">{children}</div>
    </aside>
  );
}

/** The phone panel: a sheet from the bottom, a grabber, a 20 px title, a round Close. `transport` is an editor's own play row. */
export function Sheet({
  children,
  className,
  transport,
  ...head
}: PanelProps & { transport?: React.ReactNode }) {
  return (
    <section
      role="dialog"
      aria-modal="true"
      aria-label={head.title}
      onKeyDown={(e) => {
        if (e.key !== "Escape") return;
        // The sheet owns this Escape: the shell's ladder must not run as well.
        e.stopPropagation();
        head.onClose();
      }}
      className={cn(
        "panel-sheet absolute inset-x-0 bottom-0 top-(--k7-sheet-top) z-(--k7-z-panel) flex flex-col gap-2.5 rounded-t-(--k7-r-sheet) px-4 pt-1.5 pb-[34px] animate-[k7-sheet-in_var(--k7-dur-panel)_var(--k7-ease-spring)]",
        className,
      )}
    >
      <span
        aria-hidden
        className="mx-auto mb-1 h-[5px] w-9 shrink-0 rounded-full bg-(--k7-grabber)"
      />
      <Heading {...head} sheet />
      {transport}
      <div className="min-h-0 flex-1 overflow-y-auto">{children}</div>
    </section>
  );
}
