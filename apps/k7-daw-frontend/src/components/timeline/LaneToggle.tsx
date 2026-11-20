// LaneToggle.tsx — one of a lane gutter's M/S/A buttons, the kit's TrackToggle with the gutter's rules.
// On touch it fills its share of a second line and grows an invisible 44 px-tall hit box; its
// visual size is not its target size. See docs/timeline.md, "M/S/A in the gutter".

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
        "relative size-7",
        touch &&
          "min-w-0 flex-1 after:absolute after:inset-x-0 after:-inset-y-2 after:content-['']",
        pulse && "animate-pulse",
        className,
      )}
    />
  );
}
