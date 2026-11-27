// editor-dock.tsx — the bottom editor dock of Pad and Desk (EditorDock) and its phone twin, the editor sheet.
// The dock floats OVER the lanes and never pushes them; the host positions the parent (it is `absolute`).
// Radix Dialog owns the behaviour, non-modal while the lanes stay in use (docs/editor-dock.md). The one
// thing a reader gets wrong: Escape leaves the expanded state first, so the dock's close is the SECOND press.

import { Dialog } from "radix-ui";
import * as React from "react";
import { createPortal } from "react-dom";
import { Info } from "./icons";
import { cn } from "./lib/utils";
import {
  focusTitleOnKeyboardOpen,
  PanelHeading,
  type PanelProps,
  settle,
} from "./side-panel";

interface DockProps extends PanelProps {
  expanded: boolean;
  onExpandedChange: (expanded: boolean) => void;
  /** The phone sheet's grabber's name; `aria-pressed` says whether the sheet is expanded. */
  expandLabel: string;
}

// undefined outside a dock (the row renders inline), null until the header's slot is mounted.
const SlotCtx = React.createContext<HTMLElement | null | undefined>(undefined);

// True inside the phone sheet, whose header is slimmer than the dock's.
const SheetCtx = React.createContext(false);

/** Is the editor inside the phone sheet? Its header then holds less than the dock's. */
export const useInSheet = () => React.useContext(SheetCtx);

/** An editor's first row, drawn in the dock's header beside Expand and Close; `bodyOnPhone` keeps it in the body inside the sheet. */
export function DockHeaderRow({
  children,
  bodyOnPhone = false,
}: {
  children: React.ReactNode;
  bodyOnPhone?: boolean;
}) {
  const slot = React.useContext(SlotCtx);
  const sheet = React.useContext(SheetCtx);
  if (slot === undefined || (bodyOnPhone && sheet)) return <>{children}</>;
  return slot ? createPortal(children, slot) : null;
}

/** The editor's help, one glyph in the title bar: the words are its tooltip and its name, never a line in the content. */
export function DockHint({ text }: { text: string }) {
  return (
    <DockHeaderRow>
      <span
        role="img"
        aria-label={text}
        title={text}
        data-slot="dock-hint"
        className="grid size-9 shrink-0 cursor-help place-items-center text-fg-2"
      >
        <Info className="size-4" />
      </span>
    </DockHeaderRow>
  );
}

/** What an editor says when it has nothing to edit (loading, empty, an error): a status in the title bar, the content left blank. */
export function DockStatus({
  children,
  tone = "muted",
  title,
}: {
  children: React.ReactNode;
  tone?: "muted" | "error";
  /** The tooltip: the raw detail a person is not meant to read inline. */
  title?: string;
}) {
  return (
    <DockHeaderRow>
      <p
        role="status"
        data-slot="dock-status"
        title={title}
        className={cn(
          "type-secondary min-w-0 flex-1 truncate",
          tone === "error" ? "text-(--k7-error)" : "text-fg-2",
        )}
      >
        {children}
      </p>
    </DockHeaderRow>
  );
}

const headerOf = (
  p: Omit<DockProps, "children">,
  row: React.Ref<HTMLDivElement>,
  sheet?: boolean,
) => (
  <PanelHeading
    title={p.title}
    row={row}
    onClose={p.onClose}
    closeLabel={p.closeLabel}
    sheet={sheet}
    actions={p.actions}
  />
);

/** Where the dock's top stops when expanded: under the header, or under the floating row the host names in --k7-dock-top. */
const TOP =
  "calc(var(--k7-dock-top, var(--k7-header-h)) + var(--k7-panel-gap-top))";
const ROOM = `calc(100% - ${TOP} - var(--k7-panel-bottom))`;

const px = (el: Element, name: string) =>
  Number.parseFloat(getComputedStyle(el).getPropertyValue(name)) || 0;

/** The tallest the dock goes by dragging: the expanded height. */
function tallest(dock: HTMLElement): number {
  const host = dock.offsetParent ?? dock.parentElement;
  if (!host) return window.innerHeight;
  return (
    host.clientHeight -
    (px(host, "--k7-dock-top") || px(host, "--k7-header-h")) -
    px(host, "--k7-panel-gap-top") -
    px(host, "--k7-panel-bottom")
  );
}

const STEP = 16;

/** How long a press holds before a vertical move expands or collapses instead of resizing, ms. */
const HOLD_MS = 400;
/** How far a press may wander and still count as held, px. */
const HOLD_SLOP = 6;
/** How far a held press travels before it expands or collapses, px. */
const SWIPE = 24;

/** A press on a grabber: held still, then moved up it expands and down it collapses; moved at once, `drag` takes it over. */
function holdThenSwipe(
  e: React.PointerEvent<HTMLElement>,
  onExpandedChange: (expanded: boolean) => void,
  setHeld: (held: boolean) => void,
  drag?: (dy: number) => void,
) {
  e.preventDefault();
  const handle = e.currentTarget;
  handle.setPointerCapture(e.pointerId);
  const startY = e.clientY;
  let held = false;
  let dragging = false;
  let done = false;
  const timer = setTimeout(() => {
    held = true;
    setHeld(true);
  }, HOLD_MS);
  const move = (ev: PointerEvent) => {
    const dy = ev.clientY - startY;
    if (held) {
      if (done || Math.abs(dy) < SWIPE) return;
      done = true;
      onExpandedChange(dy < 0);
      return;
    }
    if (!dragging && Math.abs(dy) > HOLD_SLOP) {
      clearTimeout(timer);
      dragging = true;
    }
    if (dragging) drag?.(dy);
  };
  const end = () => {
    clearTimeout(timer);
    setHeld(false);
    handle.removeEventListener("pointermove", move);
    handle.removeEventListener("pointerup", end);
    handle.removeEventListener("pointercancel", end);
  };
  handle.addEventListener("pointermove", move);
  handle.addEventListener("pointerup", end);
  handle.addEventListener("pointercancel", end);
}

/** The pill a grabber draws; it widens and brightens once a press is held, so the person knows a move will expand or collapse. */
function Pill({ held }: { held: boolean }) {
  return (
    <span
      aria-hidden
      className={cn(
        "h-[5px] rounded-full transition-all duration-150",
        held ? "w-14 bg-white/80" : "w-9 bg-white/35",
      )}
    />
  );
}

/** The top-edge grabber: a drag, or the arrow keys, sets the dock's height; a held press moved up or down expands or collapses it. */
function ResizeHandle({
  dock,
  height,
  onHeightChange,
  label,
  expanded,
  onExpandedChange,
}: {
  dock: React.RefObject<HTMLElement | null>;
  height: number;
  onHeightChange: (h: number) => void;
  label: string;
  expanded: boolean;
  onExpandedChange: (expanded: boolean) => void;
}) {
  const [held, setHeld] = React.useState(false);
  const set = (h: number) => {
    const el = dock.current;
    if (!el) return;
    const min = px(el, "--k7-dock-min-h") || 120;
    onHeightChange(Math.round(Math.max(min, Math.min(tallest(el), h))));
  };
  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    const el = dock.current;
    if (!el) return;
    const startH = el.offsetHeight;
    holdThenSwipe(e, onExpandedChange, setHeld, (dy) => {
      if (!expanded) set(startH - dy);
    });
  };
  const onKeyDown = (e: React.KeyboardEvent) => {
    const to: Record<string, number> = {
      ArrowUp: height + STEP,
      ArrowDown: height - STEP,
      Home: Number.POSITIVE_INFINITY,
      End: 0,
    };
    // Enter does what a held press and a move do: expand, or collapse when expanded.
    if (e.key === "Enter") {
      e.preventDefault();
      onExpandedChange(!expanded);
      return;
    }
    if (!(e.key in to)) return;
    e.preventDefault();
    set(to[e.key] ?? height);
  };
  const el = dock.current;
  return (
    // A 44 px hit area reaching 28 px above the dock, so the header's row stays clear; the pill is unchanged.
    <div
      role="separator"
      aria-orientation="horizontal"
      aria-label={label}
      aria-valuenow={Math.round(height)}
      aria-valuemin={el ? px(el, "--k7-dock-min-h") || 120 : 120}
      aria-valuemax={el ? Math.round(tallest(el)) : undefined}
      // biome-ignore lint/a11y/noNoninteractiveTabindex: a focusable separator is the WAI-ARIA window splitter.
      tabIndex={0}
      title={label}
      onPointerDown={onPointerDown}
      onKeyDown={onKeyDown}
      className="absolute inset-x-0 -top-7 mx-auto flex h-11 w-24 cursor-row-resize touch-none items-start justify-center rounded-full pt-[33px] outline-none focus-visible:shadow-(--k7-focus-ring)"
    >
      <Pill held={held} />
    </div>
  );
}

/** The dock: left and right 12, above the bar, `height` tall, or from the header down when expanded. A non-modal dialog. */
export function EditorDock({
  children,
  className,
  onEscapeKeyDown,
  height,
  onHeightChange,
  resizeLabel,
  ...head
}: DockProps & {
  /** The normal height in px; the host keeps it, and the dock never goes under the minimum nor above the expanded height. */
  height: number;
  onHeightChange: (height: number) => void;
  resizeLabel: string;
}) {
  const ref = React.useRef<HTMLDivElement>(null);
  const [slot, setSlot] = React.useState<HTMLElement | null>(null);
  return (
    <Dialog.Root
      open
      modal={false}
      onOpenChange={(o) => {
        if (o) return;
        if (head.expanded) head.onExpandedChange(false);
        else head.onClose();
      }}
    >
      <Dialog.Content
        ref={ref}
        role="complementary"
        data-slot="panel-frame"
        data-expanded={head.expanded}
        aria-describedby={undefined}
        onOpenAutoFocus={focusTitleOnKeyboardOpen}
        onInteractOutside={(e) => e.preventDefault()}
        onEscapeKeyDown={settle(onEscapeKeyDown)}
        style={
          head.expanded ? { top: TOP } : { height: `min(${height}px, ${ROOM})` }
        }
        className={cn(
          "panel absolute left-3 right-(--k7-dock-right,12px) bottom-(--k7-panel-bottom) z-(--k7-z-panel) flex flex-col gap-2.5 rounded-(--k7-r-panel) p-(--k7-dock-padding) outline-none animate-[k7-dock-in_var(--k7-dur-panel)_var(--k7-ease-spring)]",
          className,
        )}
      >
        <ResizeHandle
          dock={ref}
          height={height}
          onHeightChange={onHeightChange}
          label={resizeLabel}
          expanded={head.expanded}
          onExpandedChange={head.onExpandedChange}
        />
        {headerOf(head, setSlot)}
        <SlotCtx.Provider value={slot}>
          <div className="min-h-0 flex-1 overflow-y-auto pr-2">{children}</div>
        </SlotCtx.Provider>
      </Dialog.Content>
    </Dialog.Root>
  );
}

/** The sheet's grabber: a held press moved up expands the sheet, moved down folds it; Enter does the same. */
function SheetGrabber({
  label,
  expanded,
  onExpandedChange,
}: {
  label: string;
  expanded: boolean;
  onExpandedChange: (expanded: boolean) => void;
}) {
  const [held, setHeld] = React.useState(false);
  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={expanded}
      title={label}
      onPointerDown={(e) => holdThenSwipe(e, onExpandedChange, setHeld)}
      onKeyDown={(e) => {
        if (e.key !== "Enter" && e.key !== " ") return;
        e.preventDefault();
        onExpandedChange(!expanded);
      }}
      className="absolute inset-x-0 top-0 mx-auto flex h-11 w-24 touch-none items-start justify-center rounded-full pt-2 outline-none focus-visible:shadow-(--k7-focus-ring)"
    >
      <Pill held={held} />
    </button>
  );
}

/** The phone sheet: partial height and non-modal, so the lanes stay in view; expanded it is full screen and modal. */
export function EditorSheet({
  children,
  className,
  onEscapeKeyDown,
  ...head
}: DockProps) {
  const [slot, setSlot] = React.useState<HTMLElement | null>(null);
  return (
    <Dialog.Root
      open
      modal={head.expanded}
      onOpenChange={(o) => {
        if (o) return;
        if (head.expanded) head.onExpandedChange(false);
        else head.onClose();
      }}
    >
      <Dialog.Content
        data-slot="panel-frame"
        data-expanded={head.expanded}
        aria-describedby={undefined}
        onOpenAutoFocus={head.expanded ? undefined : focusTitleOnKeyboardOpen}
        onInteractOutside={
          head.expanded ? undefined : (e) => e.preventDefault()
        }
        onEscapeKeyDown={settle(onEscapeKeyDown)}
        className={cn(
          "panel-sheet absolute inset-x-0 bottom-0 z-(--k7-z-panel) flex flex-col gap-2.5 rounded-t-(--k7-r-sheet) px-4 pt-4 pb-[34px] outline-none animate-[k7-sheet-in_var(--k7-dur-panel)_var(--k7-ease-spring)]",
          head.expanded ? "top-(--k7-sheet-top)" : "top-(--k7-dock-sheet-top)",
          className,
        )}
      >
        <SheetGrabber
          label={head.expandLabel}
          expanded={head.expanded}
          onExpandedChange={head.onExpandedChange}
        />
        {headerOf(head, setSlot, true)}
        <SlotCtx.Provider value={slot}>
          <SheetCtx.Provider value>
            <div className="min-h-0 flex-1 overflow-y-auto">{children}</div>
          </SheetCtx.Provider>
        </SlotCtx.Provider>
      </Dialog.Content>
    </Dialog.Root>
  );
}
