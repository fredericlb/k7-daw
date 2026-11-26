// side-panel.tsx — the right panel of Pad and Desk (SidePanel) and its phone twin, the full-screen sheet.
// A panel floats OVER the panes and never pushes them; the host positions the parent (it is `absolute`).
// Radix Dialog owns the behaviour (docs/side-panel.md); the content renders in place, never in a portal.

import { Dialog } from "radix-ui";
import type * as React from "react";
import { Button } from "./button";
import { X } from "./icons";
import { cn } from "./lib/utils";

export interface PanelProps {
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
  /** Runs first on Escape; preventDefault leaves the key to the host, otherwise the panel closes and the key goes no further. */
  onEscapeKeyDown?: (e: KeyboardEvent) => void;
}

/** A panel opened from the keyboard takes focus on its title (a pointer open leaves it where it was). */
export const focusTitleOnKeyboardOpen = (e: Event) => {
  e.preventDefault();
  const from = document.activeElement;
  if (!(from instanceof HTMLElement) || !from.matches(":focus-visible")) return;
  const frame = (e.currentTarget ?? e.target) as HTMLElement | null;
  frame?.querySelector<HTMLElement>("[data-slot=panel-title]")?.focus();
};

export const settle =
  (host?: (e: KeyboardEvent) => void) => (e: KeyboardEvent) => {
    host?.(e);
    if (!e.defaultPrevented) e.stopPropagation();
  };

/** The header row both frames draw, and the editor dock's too. */
export function PanelHeading({
  title,
  subtitle,
  icon,
  actions,
  onClose,
  closeLabel,
  sheet,
  row,
}: Omit<PanelProps, "children" | "className" | "onEscapeKeyDown"> & {
  sheet?: boolean;
  /** Replaces the glyph and the visible title by an empty slot (the editor dock's first row); the title stays as the name. */
  row?: React.Ref<HTMLDivElement>;
}) {
  return (
    <div className="relative flex min-h-11 shrink-0 items-center gap-2.5">
      {row ? (
        <>
          <Dialog.Title
            tabIndex={-1}
            data-slot="panel-title"
            className="sr-only"
          >
            {title}
          </Dialog.Title>
          <div
            ref={row}
            className="flex min-w-0 flex-1 flex-wrap items-center gap-x-2 gap-y-1"
          />
        </>
      ) : null}
      {!row && icon ? (
        <span className="grid shrink-0 place-items-center text-primary-text [&_svg]:size-5">
          {icon}
        </span>
      ) : null}
      {row ? null : (
        <div className="min-w-0 flex-1">
          <Dialog.Title
            tabIndex={-1}
            data-slot="panel-title"
            className={cn(
              "truncate text-foreground outline-none",
              sheet ? "type-sheet-title" : "type-headline",
            )}
          >
            {title}
          </Dialog.Title>
          {subtitle ? (
            <p className="type-status truncate text-fg-2">{subtitle}</p>
          ) : null}
        </div>
      )}
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

/** The 400 px floating panel: right 12, under the header, clear of the bar. A non-modal dialog: the panes stay usable. */
export function SidePanel({
  children,
  className,
  onEscapeKeyDown,
  ...head
}: PanelProps) {
  return (
    <Dialog.Root
      open
      modal={false}
      onOpenChange={(o) => {
        if (!o) head.onClose();
      }}
    >
      <Dialog.Content
        role="complementary"
        data-slot="panel-frame"
        aria-describedby={undefined}
        onOpenAutoFocus={focusTitleOnKeyboardOpen}
        onInteractOutside={(e) => e.preventDefault()}
        onEscapeKeyDown={settle(onEscapeKeyDown)}
        className={cn(
          "panel absolute right-(--k7-panel-right) bottom-(--k7-panel-bottom-open,var(--k7-panel-bottom)) top-[calc(var(--k7-header-h)+var(--k7-panel-gap-top))] z-(--k7-z-panel) flex w-(--k7-panel-w) max-w-[calc(100%-24px)] flex-col gap-2.5 rounded-(--k7-r-panel) p-(--k7-panel-padding) outline-none animate-[k7-panel-in_var(--k7-dur-panel)_var(--k7-ease-spring)]",
          className,
        )}
      >
        <PanelHeading {...head} />
        <div className="min-h-0 flex-1 overflow-y-auto pr-2">{children}</div>
      </Dialog.Content>
    </Dialog.Root>
  );
}

/** The phone panel: a sheet from the bottom, a 20 px title, a round Close. `transport` is an editor's own play row. */
export function Sheet({
  children,
  className,
  transport,
  onEscapeKeyDown,
  ...head
}: PanelProps & { transport?: React.ReactNode }) {
  return (
    <Dialog.Root
      open
      onOpenChange={(o) => {
        if (!o) head.onClose();
      }}
    >
      <Dialog.Content
        data-slot="panel-frame"
        aria-describedby={undefined}
        onEscapeKeyDown={settle(onEscapeKeyDown)}
        className={cn(
          "panel-sheet absolute inset-x-0 bottom-0 top-(--k7-sheet-top) z-(--k7-z-panel) flex flex-col gap-2.5 rounded-t-(--k7-r-sheet) px-4 pt-4 pb-[34px] outline-none animate-[k7-sheet-in_var(--k7-dur-panel)_var(--k7-ease-spring)]",
          className,
        )}
      >
        <PanelHeading {...head} sheet />
        {transport}
        <div className="min-h-0 flex-1 overflow-y-auto">{children}</div>
      </Dialog.Content>
    </Dialog.Root>
  );
}
