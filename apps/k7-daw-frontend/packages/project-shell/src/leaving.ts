// leaving.ts — whether the page is on its way out: from `beforeunload` until it is plain the page stayed.
// A read waits here rather than starting while a navigation tears the page down, which WebKit refuses (docs/fileApi.md).

/** How long after `beforeunload` a page still here counts as staying: a guard's prompt was answered Stay. */
export const LEAVING_MS = 2000;

let leavingUntil = 0;

/** Marks the page as leaving; exported for the tests, which have no navigation to fire it. */
export function markLeaving(): void {
  leavingUntil = performance.now() + LEAVING_MS;
}

// A Worker has no `beforeunload`, and its reads stop with the page that owns it.
if (typeof window !== "undefined")
  window.addEventListener("beforeunload", markLeaving);

/** Null on a page that stays, so a read starts in the same tick; on one that is leaving, the wait out of LEAVING_MS. */
export function leavingWait(): Promise<void> | null {
  const wait = leavingUntil - performance.now();
  if (wait <= 0) return null;
  return new Promise<void>((r) => setTimeout(r, wait)).then(
    () => leavingWait() ?? undefined,
  );
}
