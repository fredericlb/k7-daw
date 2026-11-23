// gitPages.ts — the home's two git pages, Sync settings and Clone a repository, as addresses: a query on the home page.
// What a reader gets wrong: what a page opens with (another device's address, its name) rides beside the address, not in it.

/** `?sync=<workspace>`: the Sync settings of that workspace. */
export const SYNC_PARAM = "sync";
/** `?clone`: Clone a repository. */
export const CLONE_PARAM = "clone";

export interface ClonePrefill {
  url: string;
  name: string;
  /** Called once the clone landed: the other-devices row hides its entry at once. */
  onCloned?: () => void;
}

let prefill: ClonePrefill | null = null;

/** What the Clone page opens with, taken once. */
export function takeClonePrefill(): ClonePrefill | null {
  const p = prefill;
  prefill = null;
  return p;
}

/** The address of the page, keeping every other parameter of the home's (the unfolded workspace). */
export function pageHref(
  href: string,
  page: { sync: string } | { clone: true } | null,
): string {
  const url = new URL(href);
  url.searchParams.delete(SYNC_PARAM);
  url.searchParams.delete(CLONE_PARAM);
  if (page && "sync" in page) url.searchParams.set(SYNC_PARAM, page.sync);
  else if (page) url.searchParams.set(CLONE_PARAM, "");
  return url.href;
}

// Loaded on use: the address helpers bring the editor's buffers along, which a test of a card does not want.
const go = (href: string, replace = false): void =>
  void import("../appNav").then((nav) => nav.go(href, replace));

/** Open the Sync settings of `workspace`. */
export const openSyncSettings = (workspace: string): void =>
  go(pageHref(window.location.href, { sync: workspace }));

/** Open Clone a repository, with another device's address and name filled in when given. */
export function openClone(with_: ClonePrefill | null = null): void {
  prefill = with_;
  go(pageHref(window.location.href, { clone: true }));
}

/** Leave a git page: Back when the page was reached in this tab, the bare home otherwise. */
export function closeGitPage(): void {
  const idx = (window.history.state as { idx?: number } | null)?.idx ?? 0;
  if (idx > 0) window.history.back();
  else go(pageHref(window.location.href, null), true);
}
