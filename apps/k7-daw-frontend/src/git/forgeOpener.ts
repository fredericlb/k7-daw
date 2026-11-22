// forgeOpener.ts — how a Forgejo sign-in reaches the server's page on each host, and how it comes back to k7.
// What a reader gets wrong: the phone apps return through the app's scheme, not the loopback; docs/git.md says why.

import { engineCall } from "../settings/androidEngine";

export interface SignInWay {
  /** `app`: the server sends the browser back to `rocks.k7.daw://callback`, which the shell forwards to the front. */
  redirect?: "app";
  /** `link`: a new tab, which the desktop app and the Android app hand to the system browser; `sheet`: iOS's web sheet. */
  open: "link" | "sheet";
}

type Marked = { k7Android?: unknown; k7Ios?: unknown };

/** The way for the page held by `win`, read off the markers the hosts set before the page's own code. */
export function signInWayOf(win: object): SignInWay {
  const w = win as Marked;
  if (w.k7Android) return { redirect: "app", open: "link" };
  if (w.k7Ios) return { redirect: "app", open: "sheet" };
  return { open: "link" };
}

export const signInWay: SignInWay = signInWayOf(
  typeof window === "undefined" ? {} : window,
);

/** iOS's sheet on `url`; settles once the sheet is gone, rejected when the person closed it. */
export function openSheet(url: string): Promise<unknown> {
  return engineCall("forgeSignIn", { url });
}
