// editor-dock.tsx — the bottom editor dock of Pad and Desk (EditorDock) and its phone twin, the editor sheet.
// The dock floats OVER the lanes and never pushes them; the host positions the parent (it is `absolute`).
// Radix Dialog owns the behaviour, non-modal while the lanes stay in use (docs/editor-dock.md). The one
// thing a reader gets wrong: Escape leaves the expanded state first, so the dock's close is the SECOND press.

import { Dialog } from "radix-ui";
import * as React from "react";
import { createPortal } from "react-dom";
import { Button } from "./button";
import { Maximize2, Minimize2 } from "./icons";
import { cn } from "./lib/utils";
import { PanelHeading, type PanelProps, settle } from "./side-panel";

interface DockProps extends PanelProps {
  expanded: boolean;
  onExpandedChange: (expanded: boolean) => void;
  /** The Expand toggle's name, the same in both states: `aria-pressed` says which one it is in. */
  expandLabel: string;
}

/** The 44 round toggle between the actions and Close. */
function ExpandButton({
  expanded,
  onExpandedChange,
  expandLabel,
}: Pick<DockProps, "expanded" | "onExpandedChange" | "expandLabel">) {
  const Glyph = expanded ? Minimize2 : Maximize2;
  return (
    <Button
      variant="ghost"
      size="round"
      aria-label={expandLabel}
      title={expandLabel}
      aria-pressed={expanded}
      onClick={() => onExpandedChange(!expanded)}
      className="aria-pressed:bg-on-fill aria-pressed:text-lavender aria-pressed:shadow-(--k7-highlight)"
    >
      <Glyph className="size-[18px]" strokeWidth={2} />
    </Button>
  );
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
    actions={
      <>
        {p.actions}
        <ExpandButton
          expanded={p.expanded}
          onExpandedChange={p.onExpandedChange}
          expandLabel={p.expandLabel}
        />
      </>
    }
  />
);

/** Where the dock's top stops when expanded: under the header, as the right panel's does. */
const TOP = "calc(var(--k7-header-h) + var(--k7-panel-gap-top))";
const ROOM = `calc(100% - ${TOP} - var(--k7-panel-bottom))`;

const px = (el: Element, name: string) =>
  Number.parseFloat(getComputedStyle(el).getPropertyValue(name)) || 0;

/** The tallest the dock goes by dragging: the expanded height. */
function tallest(dock: HTMLElement): number {
  const host = dock.offsetParent ?? dock.parentElement;
  if (!host) return window.innerHeight;
  return (
    host.clientHeight -
    px(host, "--k7-header-h") -
    px(host, "--k7-panel-gap-top") -
    px(host, "--k7-panel-bottom")
  );
}

const STEP = 16;

/** The top-edge grabber: a drag, or the arrow keys, sets the dock's height. */
function ResizeHandle({
  dock,
  height,
  onHeightChange,
  label,
}: {
  dock: React.RefObject<HTMLElement | null>;
  height: number;
  onHeightChange: (h: number) => void;
  label: string;
}) {
  const set = (h: number) => {
    const el = dock.current;
    if (!el) return;
    const min = px(el, "--k7-dock-min-h") || 120;
    onHeightChange(Math.round(Math.max(min, Math.min(tallest(el), h))));
  };
  const onPointerDown = (e: React.PointerEvent<HTMLButtonElement>) => {
    const el = dock.current;
    if (!el) return;
    e.preventDefault();
    const handle = e.currentTarget;
    handle.setPointerCapture(e.pointerId);
    const startY = e.clientY;
    const startH = el.offsetHeight;
    const move = (ev: PointerEvent) => set(startH + startY - ev.clientY);
    const end = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", end);
      handle.removeEventListener("pointercancel", end);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);
  };
  const onKeyDown = (e: React.KeyboardEvent) => {
    const to: Record<string, number> = {
      ArrowUp: height + STEP,
      ArrowDown: height - STEP,
      Home: Number.POSITIVE_INFINITY,
      End: 0,
    };
    if (!(e.key in to)) return;
    e.preventDefault();
    set(to[e.key] ?? height);
  };
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onPointerDown={onPointerDown}
      onKeyDown={onKeyDown}
      className="absolute inset-x-0 top-0 mx-auto grid h-4 w-20 cursor-row-resize touch-none place-items-center rounded-full outline-none focus-visible:shadow-(--k7-focus-ring)"
    >
      <span aria-hidden className="h-[5px] w-9 rounded-full bg-white/35" />
    </button>
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
        onOpenAutoFocus={(e) => e.preventDefault()}
        onInteractOutside={(e) => e.preventDefault()}
        onEscapeKeyDown={settle(onEscapeKeyDown)}
        style={
          head.expanded ? { top: TOP } : { height: `min(${height}px, ${ROOM})` }
        }
        className={cn(
          "panel absolute inset-x-3 bottom-(--k7-panel-bottom) z-(--k7-z-panel) flex flex-col gap-2.5 rounded-(--k7-r-panel) p-(--k7-dock-padding) outline-none animate-[k7-dock-in_var(--k7-dur-panel)_var(--k7-ease-spring)]",
          className,
        )}
      >
        <ResizeHandle
          dock={ref}
          height={height}
          onHeightChange={onHeightChange}
          label={resizeLabel}
        />
        {headerOf(head, setSlot)}
        <SlotCtx.Provider value={slot}>
          <div className="min-h-0 flex-1 overflow-y-auto pr-2">{children}</div>
        </SlotCtx.Provider>
      </Dialog.Content>
    </Dialog.Root>
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
        onOpenAutoFocus={head.expanded ? undefined : (e) => e.preventDefault()}
        onInteractOutside={
          head.expanded ? undefined : (e) => e.preventDefault()
        }
        onEscapeKeyDown={settle(onEscapeKeyDown)}
        className={cn(
          "panel-sheet absolute inset-x-0 bottom-0 z-(--k7-z-panel) flex flex-col gap-2.5 rounded-t-(--k7-r-sheet) px-4 pt-1.5 pb-[34px] outline-none animate-[k7-sheet-in_var(--k7-dur-panel)_var(--k7-ease-spring)]",
          head.expanded ? "top-(--k7-sheet-top)" : "top-(--k7-dock-sheet-top)",
          className,
        )}
      >
        <span
          aria-hidden
          className="mx-auto mb-1 h-[5px] w-9 shrink-0 rounded-full bg-(--k7-grabber)"
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
