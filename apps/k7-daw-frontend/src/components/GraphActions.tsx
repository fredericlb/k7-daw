// GraphActions.tsx — the Graph pane's labelled pills (Zoom fit, Auto layout, Stack) and its output-level meter.
// They sit over the pane's top-left corner; the minimap card keeps the bottom-right, free of buttons.

import { useEngineState } from "@studio/engine-browser/store";
import { Columns3, Maximize, Network } from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import { m } from "../paraglide/messages.js";

const PILL =
  "glass type-label flex h-11 cursor-pointer items-center gap-2 rounded-full px-4 text-[14px] focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none aria-pressed:bg-on-fill aria-pressed:text-lavender";

const FLOOR_DB = -60;
const CEIL_DB = 6;

/** The song's output level as a thin bar: the one place a graph's cards say how loud the whole of it is. */
function OutputMeter() {
  const db = useEngineState((s) => Math.round(s.master));
  const fill = Math.max(0, Math.min(1, (db - FLOOR_DB) / (CEIL_DB - FLOOR_DB)));
  return (
    <div
      role="meter"
      aria-label={m["graph.output"]()}
      aria-valuemin={FLOOR_DB}
      aria-valuemax={CEIL_DB}
      aria-valuenow={Number.isFinite(db) ? db : FLOOR_DB}
      className="glass flex h-11 items-center gap-2 rounded-full px-4"
    >
      <span className="font-bold text-[12px] text-fg-2 uppercase tracking-wider">
        {m["graph.output"]()}
      </span>
      <span className="h-1.5 w-24 overflow-hidden rounded-full bg-on-fill">
        <span
          className={cn(
            "block h-full origin-left rounded-full",
            db > 0 ? "bg-meter-hot" : "bg-meter-ok",
          )}
          style={{ transform: `scaleX(${fill})`, width: "100%" }}
        />
      </span>
    </div>
  );
}

export function GraphActions({
  onFit,
  onOrganize,
  onStack,
  stacked,
}: {
  onFit: () => void;
  onOrganize?: () => void;
  onStack?: () => void;
  stacked: boolean;
}) {
  return (
    <div className="pointer-events-none absolute top-3 left-3 z-10 flex flex-wrap gap-2 [&>*]:pointer-events-auto">
      <button type="button" className={PILL} onClick={onFit}>
        <Maximize className="size-5" />
        {m["graph.zoomFit"]()}
      </button>
      {onOrganize && (
        <button type="button" className={PILL} onClick={onOrganize}>
          <Network className="size-5" />
          {m["graph.autoLayout"]()}
        </button>
      )}
      {onStack && (
        <button
          type="button"
          className={PILL}
          aria-pressed={stacked}
          onClick={onStack}
        >
          <Columns3 className="size-5" />
          {m["graph.stack"]()}
        </button>
      )}
      <OutputMeter />
    </div>
  );
}
