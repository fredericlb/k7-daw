// ChainHeader.tsx — the chain dock's header row: the lane's input menu, its monitor and width on the left;
// its M, S, A, vol and pan and the automation toggle on the right. Expand and Close are the dock's own, not drawn here.

import { Button } from "@studio/ui/button";
import { DockHeaderRow } from "@studio/ui/editor-dock";
import { Spline } from "@studio/ui/icons";
import { m } from "../paraglide/messages.js";

export function ChainHeader({
  mix,
  inputMenu,
  inputProps,
  showCurves,
  onToggleCurves,
}: {
  /** The lane's M, S, A, vol and pan, at the right (LaneMixControls.tsx). */
  mix?: React.ReactNode;
  /** The lane's input menu, first on the left (ChainInputControls.tsx). */
  inputMenu?: React.ReactNode;
  /** …and its monitor and width, right beside it. */
  inputProps?: React.ReactNode;
  showCurves: boolean;
  onToggleCurves?: () => void;
}) {
  return (
    <DockHeaderRow>
      <div className="flex min-w-0 flex-1 items-center gap-2">
        {inputMenu}
        {inputProps}
        <div className="ml-auto flex shrink-0 items-center gap-2">
          {mix}
          {onToggleCurves && (
            <Button
              variant="glass"
              size="icon-md"
              onClick={onToggleCurves}
              aria-pressed={showCurves}
              aria-label={m["chain.automation"]()}
              title={
                showCurves ? m["chain.curvesHide"]() : m["chain.curvesShow"]()
              }
            >
              <Spline className="size-4" />
            </Button>
          )}
        </div>
      </div>
    </DockHeaderRow>
  );
}
