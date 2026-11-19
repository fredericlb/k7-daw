// services.ts — where k7 answers: its origin, `app.<domain>`; a `docs.<domain>` host reads as it too. `st0.app` and its `play.` / `k7.` aliases are FORMER hosts that
// saved URLs still reach, each read as the current origin. Browser-safe: no node imports, because
// the SPAs, the file service and the supervisor all have to agree about this — docs/addresses.md.

export type StudioService = "k7";

/** The alias hosts k7 also answers on — former addresses, kept for saved URLs. */
const ALIAS_LABELS = ["play", "k7"];

/** k7's label in front of its domain, and the label of a documentation host that read as it. */
const APP_LABEL = "app";
const DOCS_LABEL = "docs";

/** The former hosts, each read as the one that replaced it — docs/addresses.md. */
const LEGACY_HOSTS: Record<string, string> = {
  "st0.app": "app.k7.rocks",
  "priv.st0.app": "app.priv.k7.rocks",
};

/**
 * k7's OWN hostname, given any studio hostname — an alias's label stripped back off, `docs.`
 * swapped for `app.`, and a former host read as its successor. Pattern-based apart from
 * LEGACY_HOSTS, so a deployment on its own domain needs nothing configured here.
 */
function k7Hostname(hostname: string): string {
  let host = hostname;
  for (const label of ALIAS_LABELS) {
    const rest = host.slice(label.length + 1);
    // Stripped only off a real domain: `k7.rocks` is a domain, not an alias of `rocks`.
    if (host.startsWith(`${label}.`) && rest.includes(".")) {
      host = rest;
      break;
    }
  }
  if (host.startsWith(`${DOCS_LABEL}.`))
    host = `${APP_LABEL}.${host.slice(DOCS_LABEL.length + 1)}`;
  return LEGACY_HOSTS[host] ?? host;
}

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

/**
 * Where a service is, given any URL on studio's origin (of any of its apps): everything
 * but that URL's origin is dropped, an alias label stripped, and every service keeps what
 * is left — they share one origin.
 */
export function serviceUrl(origin: string, _service: StudioService): string {
  let parsed: URL;
  try {
    parsed = new URL(origin);
  } catch {
    return "";
  }
  const base = k7Hostname(parsed.hostname);
  // Unchanged (port included) when nothing was stripped: k7 itself, or a loopback dev host.
  return base === parsed.hostname
    ? parsed.origin
    : `${parsed.protocol}//${base}`;
}

/** A host where services are told apart by PORT rather than subdomain — a bare dev run. */
const LOOPBACK = /^(localhost|127(\.\d{1,3}){3}|\[::1\])$/;

/** Whether `hostname` (a URL's, brackets and all for IPv6) is one where services are ports rather than subdomains. */
export const isLoopbackHost = (hostname: string): boolean =>
  LOOPBACK.test(hostname);
