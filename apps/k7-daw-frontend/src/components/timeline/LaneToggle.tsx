// LaneToggle.tsx — one of a lane gutter's M/S/A buttons, the kit's TrackToggle with the gutter's rules.
// It sits on the gutter's second line: small with a mouse, and with a coarse pointer (touch, pen) or a
// phone it gets an invisible 44 px-tall hit box. See docs/timeline.md, "M/S/A in the gutter".

import { cn } from "@studio/ui/lib/utils";
import { TrackToggle, type TrackToggleKind } from "@studio/ui/track-toggle";
import { blurOnClick } from "../../keys";

/** A lane's toggle: the kit draws the three states, this adds what a gutter needs around them. */
export function LaneToggle({
  kind,
  on,
  pulse,
  title,
  ariaLabel,
  touch,
  className,
  onToggle,
}: {
  kind: TrackToggleKind;
  on: boolean;
  /** An armed lane whose camera is rolling: armed and recording are two states. */
  pulse?: boolean;
  title: string;
  ariaLabel: string;
  /** A phone's gutter: the button takes a third of the line and a finger-sized hit box. */
  touch?: boolean;
  className?: string;
  onToggle: () => void;
}) {
  return (
    <TrackToggle
      kind={kind}
      on={on}
      title={title}
      aria-label={ariaLabel}
      // Stopped so a press does not also start a row drag or move the caret.
      onPointerDown={(e) => e.stopPropagation()}
      // dblclick is its own event and bubbles past the click handler's stop.
      onDoubleClick={(e) => e.stopPropagation()}
      onClick={(e) => {
        e.stopPropagation();
        onToggle();
        // Or the next Space re-toggles this button instead of rolling the transport — see blurOnClick.
        blurOnClick(e);
      }}
      className={cn(
        // Off touch it sits on the gutter's second line, so it is drawn to fit a 28 px row under the name.
        touch
          ? "relative size-7"
          : "relative !h-3.5 !w-6 !rounded-[3px] !text-[11px] !leading-none [@media(pointer:coarse)]:!h-4 [@media(pointer:coarse)]:!w-8 [@media(pointer:coarse)]:after:absolute [@media(pointer:coarse)]:after:-inset-y-3.5 [@media(pointer:coarse)]:after:inset-x-0 [@media(pointer:coarse)]:after:content-['']",
        touch &&
          "min-w-0 flex-1 after:absolute after:inset-x-0 after:-inset-y-2 after:content-['']",
        // A static ring stays when reduced motion stops the pulse: armed and recording must differ.
        pulse && "animate-pulse ring-2 ring-foreground",
        className,
      )}
    />
  );
}
