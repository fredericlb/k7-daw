// pairing.ts — this browser's side of LAN pairing: the key an engine gave it, sent first on every socket it
// opens. A page on the engine's own machine and the server's relay need
// none — the engine trusts both — so a key is only ever needed over the LAN. See docs/pairing.md.

const PREFIX = "k7.engine-pair.";

// Which engine answered at which address, learnt by the probe: a client opens a URL and must know whose key to show.
const engineAt = new Map<string, string>();

/** The key this browser holds for an engine, or null. Storage can be refused (a private window), and that is no key. */
export function pairKeyFor(engineId: string): string | null {
  try {
    return localStorage.getItem(PREFIX + engineId);
  } catch {
    return null;
  }
}

export function rememberPairKey(engineId: string, key: string): void {
  try {
    localStorage.setItem(PREFIX + engineId, key);
  } catch {
    /* refused: the pairing lasts as long as this page */
  }
}

/** Forgotten when the engine refuses it: the device was unpaired on the machine. */
export function forgetPairKey(engineId: string): void {
  try {
    localStorage.removeItem(PREFIX + engineId);
  } catch {
    /* nothing stored */
  }
}

/** Record that `url` reaches `engineId`; the probe calls it for every address it tries. */
export function noteEngineAt(url: string, engineId: string): void {
  engineAt.set(url, engineId);
}

/** The first frame a socket to `url` sends to prove this device, or null when it holds no key for that engine. */
export function pairFrameFor(url: string): string | null {
  const id = engineAt.get(url);
  const key = id ? pairKeyFor(id) : null;
  return key ? JSON.stringify({ type: "pair", key }) : null;
}

/** What the musician will read on the machine: the kind of device, and the browser on it. */
export function deviceName(): string {
  const ua = typeof navigator === "undefined" ? "" : navigator.userAgent;
  const device = /iPad/.test(ua)
    ? "iPad"
    : /iPhone/.test(ua)
      ? "iPhone"
      : /Android/.test(ua)
        ? "Android device"
        : /Mac OS X/.test(ua)
          ? "Mac"
          : /Windows/.test(ua)
            ? "Windows PC"
            : /Linux/.test(ua)
              ? "Linux PC"
              : "device";
  const browser = /Edg\//.test(ua)
    ? "Edge"
    : /Firefox\//.test(ua)
      ? "Firefox"
      : /Chrome\//.test(ua)
        ? "Chrome"
        : /Safari\//.test(ua)
          ? "Safari"
          : "a browser";
  return `${device} · ${browser}`;
}
