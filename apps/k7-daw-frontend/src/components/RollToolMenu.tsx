// RollToolMenu.tsx — the piano roll's tools as one menu in the panel header: Select, Create, Remove, Takes.
// Each item DISPATCHES `ui roll <mode>`, so the menu and the keys stay one verb; the roll keeps its own cursor glyphs.

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import { Check, ChevronDown } from "@studio/ui/icons";
import { ModeIcon } from "../modeIcons";
import { m } from "../paraglide/messages.js";
import { ROLL_PRESS, type RollMode } from "../rollMode";

const NAME: Record<RollMode, () => string> = {
  select: () => m["roll.tool.select"](),
  create: () => m["roll.tool.create"](),
  remove: () => m["roll.tool.remove"](),
  takes: () => m["roll.tool.takes"](),
};

export function RollToolMenu({
  modes,
  current,
  onPick,
}: {
  modes: readonly RollMode[];
  current: RollMode;
  onPick: (mode: RollMode) => void;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={m["roll.tool.label"]({ name: NAME[current]() })}
          title={ROLL_PRESS[current]()}
          className="glass type-label flex h-9 cursor-pointer items-center gap-1.5 rounded-full px-3 text-[13px] focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none"
        >
          <ModeIcon mode={current} className="size-4" />
          {NAME[current]()}
          <ChevronDown className="size-4 text-fg-2" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        {modes.map((mode) => (
          <DropdownMenuItem
            key={mode}
            title={ROLL_PRESS[mode]()}
            onSelect={() => onPick(mode)}
            className="gap-3"
          >
            <ModeIcon mode={mode} className="size-5 text-primary-text" />
            {NAME[mode]()}
            {mode === current && (
              <Check className="ml-auto size-5 text-lavender" />
            )}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
