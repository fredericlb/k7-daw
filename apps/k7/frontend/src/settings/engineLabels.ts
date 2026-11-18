// engineLabels.ts — the sentences of the engine cards (the registry's list, the owners' list, the share editor).
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
  /** What this account is to the engine — apps/k7/backend-rs/docs/engine/ownership.md. */
  yours: string;
  sharedBy: (owner: string) => string;
  server: string;
  unowned: string;
  unownedHint: string;
  share: string;
  shareHint: string;
  shareEveryone: string;
  sharePlaceholder: string;
  shareSave: string;
  shareSaved: string;
  notAnEmail: (what: string) => string;
  close: string;
  giveUp: string;
  giveUpConfirm: string;
  /** The admins' card of every owned engine — apps/k7/backend-rs/docs/engine/ownership.md. */
  ownersTitle: string;
  ownersHint: string;
  ownersNone: string;
  ownedBy: (owner: string) => string;
  sharedWith: (n: number) => string;
  offline: string;
  since: (date: string) => string;
  reset: string;
  resetConfirm: string;
}

/** The cards' sentences, read from the catalogue now. */
export function engineLabels(): EngineLabels {
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
    sharedBy: (owner) => m["media.engines.sharedBy"]({ owner }),
    server: m["media.engines.server"](),
    unowned: m["media.engines.unowned"](),
    unownedHint: m["media.engines.unownedHint"](),
    share: m["media.engines.share"](),
    shareHint: m["media.engines.shareHint"](),
    shareEveryone: m["media.engines.shareEveryone"](),
    sharePlaceholder: m["media.engines.sharePlaceholder"](),
    shareSave: m["media.engines.shareSave"](),
    shareSaved: m["media.engines.shareSaved"](),
    notAnEmail: (what) => m["media.engines.notAnEmail"]({ what }),
    close: m["media.engines.close"](),
    giveUp: m["media.engines.giveUp"](),
    giveUpConfirm: m["media.engines.giveUpConfirm"](),
    ownersTitle: m["media.engines.ownersTitle"](),
    ownersHint: m["media.engines.ownersHint"](),
    ownersNone: m["media.engines.ownersNone"](),
    ownedBy: (owner) => m["media.engines.ownedBy"]({ owner }),
    sharedWith: (n) => m["media.engines.sharedWith"]({ n: String(n) }),
    offline: m["media.engines.offline"](),
    since: (date) => m["media.engines.since"]({ date }),
    reset: m["media.engines.reset"](),
    resetConfirm: m["media.engines.resetConfirm"](),
  };
}

/** The labels, rebuilt when the locale changes. */
export function useEngineLabels(): EngineLabels {
  const { locale } = useLocale();
  // biome-ignore lint/correctness/useExhaustiveDependencies: the locale is the trigger, the messages read it themselves
  return useMemo(() => engineLabels(), [locale]);
}
