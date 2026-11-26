// RollToolBar.tsx — the piano roll's tools as a segmented radiogroup in the title bar, drawn as the waveform's views are.
// Each segment DISPATCHES `ui roll <mode>`, so the control and the keys stay one verb (rollMode.ts).

import { DockHeaderRow, useInSheet } from "@studio/ui/editor-dock";
import { cn } from "@studio/ui/lib/utils";
import { ModeIcon } from "../modeIcons";
import { m } from "../paraglide/messages.js";
import { radioKeyDown } from "../radioKeys";
import { ROLL_PRESS, type RollMode } from "../rollMode";

const NAME: Record<RollMode, () => string> = {
  select: () => m["roll.tool.select"](),
  create: () => m["roll.tool.create"](),
  remove: () => m["roll.tool.remove"](),
  takes: () => m["roll.tool.takes"](),
};

export function RollToolBar({
  modes,
  current,
  onPick,
}: {
  modes: readonly RollMode[];
  current: RollMode;
  onPick: (mode: RollMode) => void;
}) {
  // In the title bar a segment is its glyph, the word its name and tooltip; the phone sheet's body row keeps the word.
  const words = useInSheet();
  return (
    <DockHeaderRow bodyOnPhone>
      <div
        role="radiogroup"
        aria-label={m["roll.tool.group"]()}
        className="glass flex h-10 shrink-0 items-center gap-0.5 rounded-full p-[3px]"
      >
        {modes.map((mode) => (
          <button
            key={mode}
            type="button"
            role="radio"
            aria-checked={mode === current}
            aria-label={NAME[mode]()}
            tabIndex={mode === current ? 0 : -1}
            onKeyDown={(e) => radioKeyDown(e, modes, current, onPick)}
            title={`${NAME[mode]()} — ${ROLL_PRESS[mode]()}`}
            onClick={() => onPick(mode)}
            className={cn(
              "flex h-8 min-w-0 cursor-pointer items-center justify-center gap-1.5 rounded-full px-2 font-bold text-[13px] transition-colors focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none",
              words ? "flex-1" : "w-9",
              mode === current
                ? "bg-on-fill text-lavender"
                : "text-foreground/80 hover:bg-on-fill/60",
            )}
          >
            <ModeIcon mode={mode} className="size-4 shrink-0" />
            {words && <span className="truncate">{NAME[mode]()}</span>}
          </button>
        ))}
      </div>
    </DockHeaderRow>
  );
}
