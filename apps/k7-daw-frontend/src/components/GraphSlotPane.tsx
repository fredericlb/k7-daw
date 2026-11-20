// GraphSlotPane.tsx — the frame every occupant of the graph's slot wears: the shell that fills
// the pane and the title bar, drawn in the dock's header; the dock's own Close is the only one.
// The occupants supply a body and what to call it. See docs/shell.md on the slot itself.

import { DockHeaderRow } from "@studio/ui/editor-dock";

/** One frame for everything that takes the graph's place — the SHELL is the point, worn two ways: with a `bar`, or with none. */
export function GraphSlotPane({
  bar,
  aside,
  children,
}: {
  /** The strip over the body; none when absent. */
  bar?: React.ReactNode;
  /** A column pinned at the body's right, sized by itself. */
  aside?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="flex h-full min-h-0 w-full min-w-0 flex-1 flex-col">
      {bar !== undefined && (
        <DockHeaderRow>
          <div className="flex min-w-0 flex-1 items-center gap-2 text-xs text-muted-foreground">
            {bar}
          </div>
        </DockHeaderRow>
      )}
      <div className="flex min-h-0 min-w-0 flex-1">
        <div className="relative min-h-0 min-w-0 flex-1">{children}</div>
        {aside}
      </div>
    </div>
  );
}
