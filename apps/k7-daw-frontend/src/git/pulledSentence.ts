// pulledSentence.ts — the sentence of a pull's outcome, shared by the home row's badge and the header's menu.

import { m } from "../paraglide/messages.js";
import type { PullState } from "./gitTypes";

/** The sentence of a pull's outcome, none for the ones that need no word. */
export const pulledSentence = (p: PullState | null): string | null =>
  p === "up-to-date"
    ? m["git.pulledNothing"]()
    : p === "empty-remote"
      ? m["git.emptyRemote"]()
      : p === "merged"
        ? m["git.pulledMerged"]()
        : null;
