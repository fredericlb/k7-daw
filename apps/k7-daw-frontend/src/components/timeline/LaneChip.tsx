// LaneChip.tsx — the phone's collapsed lane: one chip that shows which of M/S/A are on and opens the lane menu.
// A collapsed row has no room for three buttons; the chip keeps their state readable (Q22).

import { cn } from "@studio/ui/lib/utils";
import { blurOnClick } from "../../keys";

/** The letters that are on, joined; "···" when none is, so the chip is always there to press. */
export function LaneChip({
  mute,
  solo,
  arm,
  title,
  onOpen,
}: {
  mute: boolean;
  solo: boolean;
  arm: boolean;
  title: string;
  onOpen: (x: number, y: number) => void;
}) {
  const on = [mute && "M", solo && "S", arm && "A"].filter(Boolean).join(" ");
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => {
        e.stopPropagation();
        const r = e.currentTarget.getBoundingClientRect();
        onOpen(r.left, r.bottom);
        blurOnClick(e);
      }}
      className={cn(
        "type-label relative h-6 shrink-0 rounded-full px-2 font-mono text-[11px] after:absolute after:-inset-2.5 after:content-['']",
        arm
          ? "bg-record text-ink"
          : mute
            ? "bg-(--k7-mute-on) text-ink"
            : solo
              ? "bg-(--k7-solo-on) text-ink"
              : "bg-(--k7-track-btn-off) text-foreground/70",
      )}
    >
      {on || "···"}
    </button>
  );
}
