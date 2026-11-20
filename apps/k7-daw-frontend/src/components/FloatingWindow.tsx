// FloatingWindow.tsx — a window over the layout, moved by its title bar and sized by any edge or
// corner (react-rnd). Hidden rather than unmounted when closed: what it holds (the viewport's GL
// context and its <video> elements) is exactly the state that must survive being put away.
// See docs/picture.md.

import { X } from "@studio/ui/icons";
import { type ReactNode, useEffect, useState } from "react";
import { Rnd } from "react-rnd";
import {
  clampRect,
  defaultRect,
  type FloatRect,
  MIN_H,
  MIN_W,
} from "../floatingRect";
import { useLatest } from "../hooks/useLatest";

export interface FloatingWindowProps {
  title: string;
  /** A muted line under the title (the picture's size and rate); absent draws the title alone. */
  subtitle?: string;
  /** The close button's name; defaults to "Hide <title>". */
  closeLabel?: string;
  open: boolean;
  onClose: () => void;
  /** The saved rect, or null for the first placement. */
  rect: FloatRect | null;
  /** Called once per gesture, at its end — not on every pointer move. */
  onRect: (r: FloatRect) => void;
  children: ReactNode;
}

const screen = () => ({ vw: window.innerWidth, vh: window.innerHeight });

export function FloatingWindow({
  title,
  subtitle,
  closeLabel,
  open,
  onClose,
  rect,
  onRect,
  children,
}: FloatingWindowProps) {
  const [live, setLive] = useState<FloatRect>(() => {
    const { vw, vh } = screen();
    return rect ? clampRect(rect, vw, vh) : defaultRect(vw, vh);
  });
  // The latest rect for the resize listener, which would otherwise clamp a stale one.
  const liveRef = useLatest(live);

  // Rnd bounds a gesture, not the window shrinking under it: this pulls the title bar back in reach.
  useEffect(() => {
    const onResize = () => {
      const { vw, vh } = screen();
      setLive(clampRect(liveRef.current, vw, vh));
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  const settle = (next: FloatRect) => {
    const { vw, vh } = screen();
    const r = clampRect(next, vw, vh);
    setLive(r);
    onRect(r);
  };

  return (
    <Rnd
      position={{ x: live.x, y: live.y }}
      size={{ width: live.w, height: live.h }}
      minWidth={MIN_W}
      minHeight={MIN_H}
      bounds="parent"
      dragHandleClassName="floating-window-handle"
      cancel=".floating-window-cancel"
      onDragStop={(_e, d) => settle({ ...liveRef.current, x: d.x, y: d.y })}
      onResizeStop={(_e, _dir, el, _delta, pos) =>
        settle({ x: pos.x, y: pos.y, w: el.offsetWidth, h: el.offsetHeight })
      }
      // An inline display, since Rnd's own inline `inline-block` would beat a `hidden` class.
      style={{ display: open ? "flex" : "none" }}
      className="float z-(--k7-z-floating-card) flex-col overflow-hidden rounded-[20px]"
    >
      <div
        role="dialog"
        aria-label={title}
        className="flex h-full min-h-0 w-full flex-col"
      >
        {/* The title bar is the handle: no text selection, and `touch-none` so a finger drags rather than scrolls. */}
        <div className="floating-window-handle type-status flex h-9 shrink-0 cursor-grab touch-none select-none items-center gap-2 px-3 text-fg-2 active:cursor-grabbing">
          <span className="min-w-0 flex-1 truncate">
            {title}
            {subtitle && <span className="ml-2 text-fg-3">{subtitle}</span>}
          </span>
          <button
            type="button"
            onClick={onClose}
            className="floating-window-cancel rounded p-0.5 hover:bg-accent hover:text-foreground"
            aria-label={closeLabel ?? `Hide ${title.toLowerCase()}`}
            title={closeLabel ?? `Hide ${title.toLowerCase()}`}
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
        <div className="flex min-h-0 min-w-0 flex-1">{children}</div>
      </div>
    </Rnd>
  );
}
