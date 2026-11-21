import * as React from "react";
import { cn } from "./lib/utils";

export type TrackToggleKind = "mute" | "solo" | "arm";

const LETTER = { mute: "M", solo: "S", arm: "A" } as const;

// On state per kind: mute amber, solo lavender, arm the record red; the letter turns ink on a light fill.
const ON = {
  mute: "bg-(--k7-mute-on) text-ink",
  solo: "bg-(--k7-solo-on) text-ink",
  arm: "bg-record text-ink",
} as const;

// One M / S / A button for the lane header and the mixer strip: the same state in both places.
export const TrackToggle = React.forwardRef<
  HTMLButtonElement,
  Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, "children"> & {
    kind: TrackToggleKind;
    on?: boolean;
  }
>(({ kind, on = false, className, type = "button", ...props }, ref) => (
  <button
    ref={ref}
    type={type}
    aria-pressed={on}
    className={cn(
      "type-label grid size-[34px] shrink-0 cursor-pointer place-items-center rounded-(--k7-r-track-btn) outline-none transition-colors focus-visible:shadow-(--k7-focus-ring) disabled:pointer-events-none disabled:opacity-40",
      on
        ? ON[kind]
        : "bg-(--k7-track-btn-off) text-foreground shadow-[inset_0_0_0_1px_var(--k7-track-btn-line)] hover:bg-on-fill",
      className,
    )}
    {...props}
  >
    {LETTER[kind]}
  </button>
));
TrackToggle.displayName = "TrackToggle";
