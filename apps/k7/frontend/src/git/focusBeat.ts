// focusBeat.ts — the page's word to the fetch: in front now, again with each state poll, away when hidden.
// What a reader gets wrong: the host forgets a page silent for two fetch periods, so one `true` is not enough.

import { pollWhileVisible } from "../visiblePoll";

/** The page as the beat sees it, so a test needs no document. */
export interface Page {
  visible: () => boolean;
  onChange: (cb: () => void) => () => void;
}

export const documentPage: Page = {
  visible: () => document.visibilityState === "visible",
  onChange: (cb) => {
    document.addEventListener("visibilitychange", cb);
    return () => document.removeEventListener("visibilitychange", cb);
  },
};

/** Tell `send(front, opened)` now and every `everyMs` while visible; stops, and says away, on the returned call. */
export function focusBeat(
  send: (front: boolean, opened: boolean) => unknown,
  everyMs: number,
  page: Page = documentPage,
): () => void {
  const quiet = (front: boolean, opened: boolean) => {
    try {
      const r = send(front, opened);
      if (r instanceof Promise) r.catch(() => undefined);
    } catch {
      // The fetch is a convenience: a failed word is said again with the next beat.
    }
  };
  quiet(page.visible(), true);
  const off = page.onChange(() => quiet(page.visible(), false));
  const stop = pollWhileVisible(() => quiet(true, false), everyMs, {
    immediate: false,
  });
  return () => {
    off();
    stop();
    quiet(false, false);
  };
}
