// engineWords.tsx — what the engine slot SAYS: where the sound comes out, the hint and the icon,
// for the project bar and the home pages' bar alike, so the two never word the engine two ways.
// See ../../docs/shell.md.

import { Loader2, Volume2, VolumeX } from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import type { ReactNode } from "react";
import { m } from "../paraglide/messages.js";
import type { RankedEngine } from "../types";

/** What the slot shows — App's live session in a project, `HomeEngine`'s probe on the home pages. */
export interface EngineSelectorProps {
  current: RankedEngine | null;
  /** The engine is still being looked for. */
  searching: boolean;
  /** The engine's sound is played by this page rather than by its own machine. */
  audioToBrowser?: boolean;
  /** The figures beside the name — the session's link in a project, the probe's elsewhere. */
  stats?: ReactNode;
}

/** The engine's name as the slot says it: its machine. */
export function engineName(r: RankedEngine): string {
  return r.engine.hostname || r.engine.id.slice(0, 8);
}

/** What the slot says and shows. */
export function engineWords({
  current,
  searching,
  audioToBrowser = false,
}: Pick<EngineSelectorProps, "current" | "searching" | "audioToBrowser">) {
  // Where the sound comes out, said in words: a laptop driving the desktop at home is otherwise silent with no reason given.
  const label = current
    ? audioToBrowser
      ? m["engineSwitch.soundHere"]()
      : m["engineSwitch.soundOn"]({ engine: engineName(current) })
    : searching
      ? m["engines.searching"]()
      : m["engineSwitch.noSound"]();
  const hint = current
    ? audioToBrowser
      ? m["engineSwitch.hintHere"]({ engine: engineName(current) })
      : m["engineSwitch.hint"]({ engine: engineName(current) })
    : m["engineSwitch.noSoundHint"]();
  const Icon = current ? Volume2 : searching ? Loader2 : VolumeX;
  const icon = (
    <Icon
      className={cn(
        "h-4 w-4 shrink-0",
        !current && searching && "animate-spin",
        !current && !searching && "text-amber-500",
      )}
    />
  );
  return { label, hint, icon };
}
