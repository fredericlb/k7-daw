// services.ts — where k7 answers: the origin the page was served from, and the loopback ports a host may move.
// Browser-safe: no node imports, because the SPAs, the file service and the supervisor all have to agree about this — docs/addresses.md.

export type StudioService = "k7";

/** Each backend's port — what a SERVER calls, since node does not trust the dev CA; see docs/addresses.md. */
export const SERVICE_PORTS: Record<StudioService, number> = {
  k7: 8789,
};

/** How a host moves the loopback ports, as `k7=18789`: an env var for node, a global for a page. */
export const PORTS_ENV = "STUDIO_PORTS";
export const PORTS_GLOBAL = "__STUDIO_PORTS__";

/** `k7=18789` as a partial table; an unknown name or a bad number is dropped. */
export function parsePorts(
  spec: string,
): Partial<Record<StudioService, number>> {
  const out: Partial<Record<StudioService, number>> = {};
  for (const pair of spec.split(",")) {
    const [name, value] = pair.split("=").map((x) => x.trim());
    const port = Number(value);
    if (
      name in SERVICE_PORTS &&
      Number.isInteger(port) &&
      port > 0 &&
      port < 65536
    )
      out[name as StudioService] = port;
  }
  return out;
}

/** SERVICE_PORTS, with what the host moved — a phone app runs on its own port. */
export function servicePorts(): Record<StudioService, number> {
  const g = globalThis as {
    [PORTS_GLOBAL]?: unknown;
    process?: { env?: Record<string, string | undefined> };
  };
  const fromPage = g[PORTS_GLOBAL];
  const spec =
    typeof fromPage === "string" ? fromPage : g.process?.env?.[PORTS_ENV];
  return spec ? { ...SERVICE_PORTS, ...parsePorts(spec) } : SERVICE_PORTS;
}

/** Where a service answers on the machine it runs on. See SERVICE_PORTS for when to prefer it. */
export function localServiceUrl(service: StudioService): string {
  return `http://127.0.0.1:${servicePorts()[service]}`;
}

/** Where a service is, given any URL on its origin: that URL's origin, port included; "" for a string that is no URL. */
export function serviceUrl(origin: string, _service: StudioService): string {
  try {
    return new URL(origin).origin;
  } catch {
    return "";
  }
}

/** A host where services are told apart by PORT rather than subdomain — a bare dev run. */
const LOOPBACK = /^(localhost|127(\.\d{1,3}){3}|\[::1\])$/;

/** Whether `hostname` (a URL's, brackets and all for IPv6) is one where services are ports rather than subdomains. */
export const isLoopbackHost = (hostname: string): boolean =>
  LOOPBACK.test(hostname);
