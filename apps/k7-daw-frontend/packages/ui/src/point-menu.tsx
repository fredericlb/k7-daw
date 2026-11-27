// point-menu.tsx — a menu opened AT a point (a right-click, a long press, a ⋮): dismissal, edge clamp and keys.
// Radix menus anchor to a trigger; these have none, so one hook owns the contract and every pointer menu shares it (docs/radix.md).
// The one thing a reader would get wrong: `useDismiss` listens in the CAPTURE phase, so an outside press closes before it acts.

import * as React from "react";
import { reasonOf, showReason } from "./disabled-reason";
import { cn } from "./lib/utils";
import { useModalFocus } from "./modal-focus";

/** Close on a press outside `ref`, a scroll outside it, the window losing focus and Escape (which `onEscape` may take for itself by answering true). */
export function useDismiss(
  ref: React.RefObject<HTMLElement | null>,
  onClose: () => void,
  { onEscape }: { onEscape?: () => boolean | undefined } = {},
): void {
  // Read through a ref: a caller's fresh closure must not re-bind four window listeners.
  const latest = React.useRef({ onClose, onEscape });
  latest.current = { onClose, onEscape };
  React.useEffect(() => {
    const close = () => latest.current.onClose();
    const outside = (e: Event) => {
      if (!ref.current?.contains(e.target as Node)) close();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      if (!latest.current.onEscape?.()) close();
    };
    window.addEventListener("pointerdown", outside, true);
    window.addEventListener("keydown", key, true);
    window.addEventListener("blur", close);
    // The menu's own overflow scroll must not close it; a scroll does not bubble, so the capture phase sees the target.
    window.addEventListener("scroll", outside, true);
    return () => {
      window.removeEventListener("pointerdown", outside, true);
      window.removeEventListener("keydown", key, true);
      window.removeEventListener("blur", close);
      window.removeEventListener("scroll", outside, true);
    };
  }, [ref]);
}

/** Where a box of this size opens from (x, y) and still sits 8 px inside the viewport; re-measured when `deps` change. */
export function usePointClamp(
  ref: React.RefObject<HTMLElement | null>,
  x: number,
  y: number,
  ...deps: unknown[]
): { x: number; y: number } {
  const [pos, setPos] = React.useState({ x, y });
  // `deps` are the caller's triggers — the size is read from the DOM.
  React.useLayoutEffect(() => {
    const r = ref.current?.getBoundingClientRect();
    if (!r) return;
    setPos({
      x: Math.max(8, Math.min(x, window.innerWidth - r.width - 8)),
      y: Math.max(8, Math.min(y, window.innerHeight - r.height - 8)),
    });
  }, [ref, x, y, ...deps]);
  return pos;
}

/** Where Up, Down, Home and End move the focus among `n` rows from `at` (-1: none), or null for another key; wraps. */
export function stepFocus(key: string, at: number, n: number): number | null {
  if (n === 0) return null;
  if (key === "ArrowDown") return at < 0 ? 0 : (at + 1) % n;
  if (key === "ArrowUp") return at < 0 ? n - 1 : (at - 1 + n) % n;
  if (key === "Home") return 0;
  if (key === "End") return n - 1;
  return null;
}

const CloseCtx = React.createContext<() => void>(() => {});

export function PointMenu({
  x,
  y,
  onClose,
  label,
  children,
  className,
  refreshKey,
  moveFocus = stepFocus,
  onKey,
  onPointerLeave,
}: {
  /** Viewport coordinates of what opened it. */
  x: number;
  y: number;
  onClose: () => void;
  /** The menu's accessible name. */
  label?: string;
  children: React.ReactNode;
  className?: string;
  /** A new value re-clamps the box and puts the keyboard back on the first row (another tab is another height). */
  refreshKey?: unknown;
  /** The focus rule, when the host has one of its own (the app's comes from Rust). */
  moveFocus?: (key: string, at: number, n: number) => number | null;
  /** Called first with every key; answering true means it took the key. */
  onKey?: (e: KeyboardEvent) => boolean | undefined;
  onPointerLeave?: () => void;
}) {
  const ref = React.useRef<HTMLDivElement>(null);
  useModalFocus(ref, { trap: false });
  useDismiss(ref, onClose);
  const pos = usePointClamp(ref, x, y, refreshKey);
  const latest = React.useRef({ moveFocus, onKey });
  latest.current = { moveFocus, onKey };

  // biome-ignore lint/correctness/useExhaustiveDependencies: `refreshKey` is a trigger — the rows are read from the DOM.
  React.useEffect(() => {
    ref.current
      ?.querySelector<HTMLElement>('[role="menuitem"]')
      ?.focus({ preventScroll: true });
  }, [refreshKey]);

  React.useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") return;
      if (latest.current.onKey?.(e)) {
        e.stopPropagation();
        e.preventDefault();
        return;
      }
      const rows = [
        ...(ref.current?.querySelectorAll<HTMLElement>(
          '[role="menuitem"]:not([aria-disabled="true"])',
        ) ?? []),
      ];
      const at = rows.indexOf(document.activeElement as HTMLElement);
      // Enter picks the focused row here, before any shell key handler can read it as its own.
      if (e.key === "Enter" && at >= 0) {
        e.stopPropagation();
        e.preventDefault();
        rows[at]?.click();
        return;
      }
      const to = latest.current.moveFocus(e.key, at, rows.length);
      if (to === null) return;
      e.stopPropagation();
      e.preventDefault();
      rows[to]?.focus();
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, []);

  return (
    <CloseCtx.Provider value={onClose}>
      <div
        ref={ref}
        role="menu"
        aria-label={label}
        style={{ left: pos.x, top: pos.y }}
        onContextMenu={(e) => e.preventDefault()}
        onPointerLeave={onPointerLeave}
        // Bounded and scrollable: a long menu that runs off the bottom of the screen hides its own tail.
        className={cn(
          "menu fixed z-(--k7-z-menu) max-h-[70vh] min-w-[12rem] max-w-[min(26rem,calc(100vw-16px))] overflow-y-auto rounded-(--k7-r-menu) p-1.5 text-sm",
          className,
        )}
      >
        {children}
      </div>
    </CloseCtx.Provider>
  );
}

/** A thin rule between the safe rows and the destructive ones. */
export function PointMenuSeparator() {
  return <hr className="my-1 border-(--k7-hairline-row)" />;
}

export function PointMenuItem({
  danger,
  icon: Icon,
  gutter,
  shortcut,
  disabled,
  disabledReason,
  onSelect,
  children,
  className,
  ...rest
}: Omit<React.ComponentProps<"button">, "onClick"> & {
  /** Drawn in the error colour: this one does not come back. */
  danger?: boolean;
  icon?: React.ComponentType<{ className?: string }>;
  /** Hold the icon's width without an icon, so the texts of a menu line up. */
  gutter?: boolean;
  /** The key it answers to, drawn as a keycap. */
  shortcut?: string;
  /** Why it is disabled: the row then stays focusable and picking it says why. */
  disabledReason?: string;
  /** The pick; the menu closes after it. */
  onSelect: () => void;
}) {
  const close = React.useContext(CloseCtx);
  const reason = reasonOf(disabled, disabledReason);
  return (
    <button
      type="button"
      role="menuitem"
      {...rest}
      // A reasoned row keeps focus (aria-disabled); an unreasoned one is natively disabled.
      disabled={disabled && !reason}
      aria-disabled={reason ? true : undefined}
      title={rest.title ?? reason}
      onClick={() => {
        if (reason) return showReason(reason);
        onSelect();
        close();
      }}
      className={cn(
        "type-label flex min-h-11 w-full cursor-pointer items-center gap-2 rounded-2xl px-3 text-left outline-none hover:bg-on-fill focus-visible:bg-on-fill desk:min-h-8 disabled:cursor-default disabled:opacity-40 aria-disabled:opacity-40",
        danger ? "text-error" : "hover:text-fg-1 focus-visible:text-fg-1",
        className,
      )}
    >
      {Icon || gutter ? (
        <span className="flex size-4 shrink-0 items-center justify-center">
          {Icon ? <Icon className="size-4" /> : null}
        </span>
      ) : null}
      {typeof children === "string" ? (
        <span className="min-w-0 flex-1 truncate">{children}</span>
      ) : (
        children
      )}
      {shortcut ? (
        <kbd className="shrink-0 rounded border border-hairline px-1 font-mono text-[11px] text-fg-2">
          {shortcut}
        </kbd>
      ) : null}
    </button>
  );
}
