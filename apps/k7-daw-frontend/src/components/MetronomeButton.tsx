// MetronomeButton.tsx — the bar's click toggle, with its LEVEL under a right-click or a press and hold. Two
// questions on one control: whether the metronome sounds (the click) and how loud (the menu).
// Both are project settings written to settings.k7x; see @studio/song docs/metronome.md.

import { METRONOME_MIN } from "@studio/engine-protocol/build";
import { CapsuleButton } from "@studio/ui/capsule";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import {
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  useEffect,
  useRef,
  useState,
} from "react";
import { m } from "../paraglide/messages.js";
import { MetronomeIcon } from "../shellMeta";

/** A touch screen as the main pointer: there the level is behind a press and hold, not a right click. */
const coarse = (): boolean =>
  typeof matchMedia === "function" && matchMedia("(pointer: coarse)").matches;

export function MetronomeButton({
  on,
  level,
  onToggle,
  onLevel,
}: {
  on: boolean;
  /** 0.2 … 1, from the compiled doc. */
  level: number;
  onToggle: () => void;
  /** Commit a new level — one command, so one line in the journal and one Ctrl+Z. */
  onLevel: (level: number) => void;
}) {
  const [open, setOpen] = useState(false);
  // A press and hold opens the level on touch, where there is no right click; the release then is not a toggle.
  const holdTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const held = useRef(false);
  const endHold = () => {
    if (holdTimer.current) clearTimeout(holdTimer.current);
    holdTimer.current = null;
  };
  /** What the slider shows while it is being dragged — the command rewrites settings.k7x, so it goes on RELEASE. */
  const [dragging, setDragging] = useState<number | null>(null);
  const shown = dragging ?? level;
  // A reload that changes the level while the menu sits open should be seen, not overwritten by a ghost nobody is holding.
  useEffect(() => {
    if (!open) setDragging(null);
  }, [open]);

  const commit = () => {
    if (dragging === null) return;
    const v = dragging;
    setDragging(null);
    // Only when it actually moved: reopening the menu and letting go must not write the file.
    if (Math.abs(v - level) > 1e-6) onLevel(v);
  };

  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      {/* The trigger ANCHORS the menu and never opens it — `onContextMenu` does, so a left click is the toggle and nothing else. */}
      <DropdownMenuTrigger asChild>
        <CapsuleButton
          pressed={on}
          onClick={() => {
            if (held.current) return;
            onToggle();
          }}
          // Radix's trigger opens on POINTERDOWN, so a left click would toggle and open the menu at once; a cancelled pointerdown skips it and leaves the click.
          onPointerDown={(e: ReactPointerEvent) => {
            e.preventDefault();
            held.current = false;
            if (e.pointerType === "mouse") return;
            holdTimer.current = setTimeout(() => {
              held.current = true;
              setOpen(true);
            }, 500);
          }}
          onPointerUp={endHold}
          onPointerLeave={endHold}
          onKeyDown={(e: ReactKeyboardEvent) => {
            // Its own two keys, for the same reason; ArrowDown is left to the trigger, as the way to the level without a mouse.
            if (e.key !== "Enter" && e.key !== " ") return;
            e.preventDefault();
            // A held key repeats, and a toggle repeated forty times a second is a coin flip.
            if (!e.repeat) onToggle();
          }}
          onContextMenu={(e: ReactMouseEvent) => {
            e.preventDefault();
            setOpen(true);
          }}
          title={m[coarse() ? "metro.hintTouch" : "metro.hint"]({
            name: m["transport.metronome"](),
            level: Math.round(level * 100),
          })}
          aria-label={m["transport.metronome"]()}
        >
          <MetronomeIcon />
        </CapsuleButton>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-56 p-3">
        <label className="flex items-center gap-2 text-xs">
          {m["metro.volume"]()}
          <input
            type="range"
            min={METRONOME_MIN}
            max={1}
            step={0.05}
            value={shown}
            onChange={(e) => setDragging(Number(e.target.value))}
            onPointerUp={commit}
            // The keyboard moves it too, and a key press is its own release: without this the arrows would paint and write nothing.
            onKeyUp={commit}
            onBlur={commit}
            className="flex-1"
            aria-label={m["metro.volumeLabel"]()}
          />
          <span className="w-8 text-right tabular-nums">
            {Math.round(shown * 100)}%
          </span>
        </label>
        {/* Said out loud, because a slider stopping short of zero otherwise looks like a bug. */}
        <p className="mt-2 text-[11px] text-muted-foreground leading-snug">
          {m["metro.floorHint"]({ min: Math.round(METRONOME_MIN * 100) })}
        </p>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
