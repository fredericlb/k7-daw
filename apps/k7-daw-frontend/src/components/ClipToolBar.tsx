// ClipToolBar.tsx — the clip editor's tool axis as the kit Choice in the title bar: Range, Split, Stretch, List, Takes.
// The ids are the wave modes (waveMode.ts); only their words changed, so `ui wave <mode>` keeps meaning what it did.

import { Choice } from "@studio/ui/choice";
import { DockHeaderRow, useInSheet } from "@studio/ui/editor-dock";
import { List } from "@studio/ui/icons";
import { ModeIcon } from "../modeIcons";
import { m } from "../paraglide/messages.js";
import { WAVE_PRESS, type WaveMode } from "../waveMode";

const LABEL: Record<WaveMode, () => string> = {
  standard: () => m["clip.tool.range"](),
  onsets: () => m["clip.tool.split"](),
  warp: () => m["clip.tool.stretch"](),
  list: () => m["clip.tool.list"](),
  takes: () => m["clip.tool.takes"](),
};

/** One segment per mode; the press DISPATCHES the command (`onPick`), so the control and its verb never drift. */
export function ClipToolBar({
  modes,
  current,
  onPick,
}: {
  modes: readonly WaveMode[];
  current: WaveMode;
  onPick: (mode: WaveMode) => void;
}) {
  // In the title bar a segment is its glyph, the word its name and tooltip; the phone sheet's body row keeps the word.
  const words = useInSheet();
  return (
    <DockHeaderRow bodyOnPhone>
      <Choice
        label={m["clip.viewLabel"]()}
        iconOnly={!words}
        className="shrink-0"
        value={current}
        onValueChange={onPick}
        options={modes.map((mode) => ({
          value: mode,
          label: LABEL[mode](),
          title: `${LABEL[mode]()} — ${WAVE_PRESS[mode]}`,
          icon: mode === "list" ? <List /> : <ModeIcon mode={mode} />,
        }))}
      />
    </DockHeaderRow>
  );
}
