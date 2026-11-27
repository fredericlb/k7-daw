// RollToolBar.tsx — the piano roll's tools as the kit Choice in the title bar.
// Each segment DISPATCHES `ui roll <mode>`, so the control and the keys stay one verb (rollMode.ts).

import { Choice } from "@studio/ui/choice";
import { DockHeaderRow, useInSheet } from "@studio/ui/editor-dock";
import { ModeIcon } from "../modeIcons";
import { m } from "../paraglide/messages.js";
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
      <Choice
        label={m["roll.tool.group"]()}
        iconOnly={!words}
        className="shrink-0"
        value={current}
        onValueChange={onPick}
        options={modes.map((mode) => ({
          value: mode,
          label: NAME[mode](),
          title: `${NAME[mode]()} — ${ROLL_PRESS[mode]()}`,
          icon: <ModeIcon mode={mode} />,
        }))}
      />
    </DockHeaderRow>
  );
}
