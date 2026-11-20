// ClipToolBar.tsx — the clip editor's tool axis as a 40 px segmented radiogroup: Range, Split, Stretch, List, Takes.
// The ids are the wave modes (waveMode.ts); only their words changed, so `ui wave <mode>` keeps meaning what it did.

import { List } from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
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
  return (
    <div
      role="radiogroup"
      aria-label={m["clip.viewLabel"]()}
      className="flex h-10 shrink-0 items-center gap-0.5 glass rounded-full p-[3px]"
    >
      {modes.map((mode) => (
        <button
          key={mode}
          type="button"
          role="radio"
          aria-checked={mode === current}
          title={`${LABEL[mode]()} — ${WAVE_PRESS[mode]}`}
          onClick={() => onPick(mode)}
          className={cn(
            "flex h-8 min-w-0 flex-1 cursor-pointer items-center justify-center gap-1.5 rounded-full px-2 font-bold text-[13px] transition-colors focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none",
            mode === current
              ? "bg-on-fill text-lavender"
              : "text-foreground/80 hover:bg-on-fill/60",
          )}
        >
          {mode === "list" ? (
            <List className="size-4 shrink-0" />
          ) : (
            <ModeIcon mode={mode} className="size-4 shrink-0" />
          )}
          <span className="truncate">{LABEL[mode]()}</span>
        </button>
      ))}
    </div>
  );
}
