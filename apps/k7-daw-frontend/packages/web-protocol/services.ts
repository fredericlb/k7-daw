// services.ts — where k7 answers: the origin the page was served from.
// Browser-safe: no node imports, because the SPAs, the file service and the supervisor all have to agree about this — docs/addresses.md.

export type StudioService = "k7";

/** The global a host sets, before the page's scripts run, to move the loopback ports as `k7=18789`. */
export const PORTS_GLOBAL = "__STUDIO_PORTS__";

/** Where a service is, given any URL on its origin: that URL's origin, port included; "" for a string that is no URL. */
export function serviceUrl(origin: string, _service: StudioService): string {
  try {
    return new URL(origin).origin;
  } catch {
    return "";
  }
}
