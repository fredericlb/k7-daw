// ChainHeader.tsx — the chain dock's header row: the lane's input menu on the left; its monitor and width,
// Arm and the automation toggle on the right. Expand and Close are the dock's own, not drawn here.

import { DockHeaderRow } from "@studio/ui/editor-dock";
import { Spline } from "@studio/ui/icons";
import { TrackToggle } from "@studio/ui/track-toggle";
import { m } from "../paraglide/messages.js";

// A 36 px glass chip, the shape the kit gives a toggle inside a panel.
const CHIP =
  "glass grid h-9 min-w-9 cursor-pointer place-items-center rounded-full px-2 text-foreground focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none";

export function ChainHeader({
  name,
  armed,
  onArm,
  inputMenu,
  inputProps,
  showCurves,
  onToggleCurves,
}: {
  name: string;
  armed?: boolean;
  onArm?: () => void;
  /** The lane's input menu, first on the left (ChainInputControls.tsx). */
  inputMenu?: React.ReactNode;
  /** …and its monitor and width, among the properties on the right. */
  inputProps?: React.ReactNode;
  showCurves: boolean;
  onToggleCurves?: () => void;
}) {
  return (
    <DockHeaderRow>
      <div className="flex min-w-0 flex-1 items-center gap-2">
        {inputMenu}
        <div className="ml-auto flex shrink-0 items-center gap-2">
          {inputProps}
          {onArm && (
            <TrackToggle
              kind="arm"
              on={armed}
              aria-label={m["chain.armInput"]({ name })}
              title={m["chain.armInput"]({ name })}
              onClick={onArm}
            />
          )}
          {onToggleCurves && (
            <button
              type="button"
              onClick={onToggleCurves}
              aria-pressed={showCurves}
              aria-label={m["chain.automation"]()}
              title={
                showCurves ? m["chain.curvesHide"]() : m["chain.curvesShow"]()
              }
              className={`${CHIP} aria-pressed:bg-on-fill aria-pressed:text-lavender`}
            >
              <Spline className="size-4" />
            </button>
          )}
        </div>
      </div>
    </DockHeaderRow>
  );
}
