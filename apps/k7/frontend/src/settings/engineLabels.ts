// engineLabels.ts — the sentences of the engine card (the registry's list).
// Built per render so a locale switch re-words them; the message keys keep their `media.engines.*` names.

import { useMemo } from "react";
import { useLocale } from "../i18n";
import { m } from "../paraglide/messages.js";

export interface EngineLabels {
  title: string;
  hint: string;
  refresh: string;
  looking: string;
  none: (origin: string) => string;
  chosen: string;
  unreachable: string;
  sameNetwork: string;
  someone: string;
  heldBy: (who: string) => string;
  /** Whether this account may use the engine — apps/k7/backend-rs/docs/engine/ownership.md. */
  yours: string;
  unowned: string;
  unownedHint: string;
}

/** The cards' sentences, read from the catalogue now. */
function engineLabels(): EngineLabels {
  return {
    title: m["media.engines.title"](),
    hint: m["media.engines.hint"](),
    refresh: m["media.engines.refresh"](),
    looking: m["media.engines.looking"](),
    none: (origin) => m["media.engines.none"]({ origin }),
    chosen: m["media.engines.chosen"](),
    unreachable: m["media.engines.unreachable"](),
    sameNetwork: m["media.engines.sameNetwork"](),
    someone: m["media.engines.someone"](),
    heldBy: (who) => m["media.engines.heldBy"]({ who }),
    yours: m["media.engines.yours"](),
    unowned: m["media.engines.unowned"](),
    unownedHint: m["media.engines.unownedHint"](),
  };
}

/** The labels, rebuilt when the locale changes. */
export function useEngineLabels(): EngineLabels {
  const { locale } = useLocale();
  // biome-ignore lint/correctness/useExhaustiveDependencies: the locale is the trigger, the messages read it themselves
  return useMemo(() => engineLabels(), [locale]);
}
