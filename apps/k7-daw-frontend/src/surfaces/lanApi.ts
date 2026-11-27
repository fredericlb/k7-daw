// lanApi.ts — this device's own front, asked about the other devices on the network. Every call is to loopback:
// the front relays to a peer's door and holds its token, which the page never sees (apps/k7-daw-backend/docs/lan.md).

import type { LanPeer } from "@studio/web-protocol";

/** The door's version this build speaks; raised with the backend's `API` whenever a file route a surface uses changes shape. */
export const LAN_API = 1;

/** A device found on the network, as the relay resolved it: GENERATED from the Rust `PeerView`. */
export type { LanPeer };

/** What a device's door says of itself: its project, the surfaces that declares, and the screens other devices show. */
export interface LanHello {
  device: {
    name: string;
    engineId: string;
    build: string | null;
    api: number;
    proto: number;
  };
  project: {
    workspace: string;
    path: string;
    label: string;
    app: string;
  } | null;
  surfaces: { id: string; type: string }[];
  screens: { surface: string; device: string; since: number }[];
}

/** What a hello is given before the row says "Not answering". */
const HELLO_MS = 3000;

async function ask<T>(path: string, ms = HELLO_MS): Promise<T> {
  const res = await fetch(path, {
    credentials: "include",
    signal: AbortSignal.timeout(ms),
    cache: "no-store",
  });
  if (!res.ok) throw new Error(`${path}: ${res.status}`);
  return (await res.json()) as T;
}

const enc = encodeURIComponent;

export const lanApi = {
  peers: () => ask<{ peers: LanPeer[] }>("/api/lan/peers", 8000),
  self: () =>
    ask<{ name: string | null; engineId: string | null }>("/api/lan/self"),
  hello: (engineId: string) => ask<LanHello>(`/api/lan/${enc(engineId)}/hello`),
  /** The screens other devices show of THIS device's project. */
  screens: () =>
    ask<{ screens: { surface: string; device: string; since: number }[] }>(
      "/api/lan/screens",
    ),
  /** A beat of presence: this device shows `surface` of the project on `engineId`. */
  beat: (engineId: string, screen: string, surface: string, device: string) =>
    fetch(`/api/lan/${enc(engineId)}/screens/${enc(screen)}`, {
      method: "PUT",
      credentials: "include",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ surface, device }),
      signal: AbortSignal.timeout(HELLO_MS),
    }).then((r) => r.ok),
  /** Says goodbye as the page goes: a beacon survives the unload a fetch would not. */
  bye: (engineId: string, screen: string): boolean =>
    navigator.sendBeacon(
      `/api/lan/${enc(engineId)}/screens/${enc(screen)}/bye`,
    ),
};
