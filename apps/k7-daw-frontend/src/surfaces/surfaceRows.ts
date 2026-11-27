// surfaceRows.ts — the Surfaces tab's rows, as data: one group per device, a row per surface, each an address or the one reason it is not.
// The filters and their order are Rust's (the chains crate's surface_rows); here the devices are sorted by locale and each address is spelled.
// See ../../docs/external.md.

import {
  ENGINE_PROTOCOL,
  MIN_ENGINE_PROTOCOL,
} from "@studio/engine-protocol/version";
import {
  type SurfaceRow as RowAnswer,
  type RowBlocked,
  type RowGroup,
  surfaceGroupsOf,
} from "@studio/k7-wasm/surfaces";
import { lanSurfaceUrl } from "@studio/web-protocol/projects";
import { LAN_API, type LanHello, type LanPeer } from "./lanApi";

/** A peer and its hello: `undefined` while the first is asked (not drawn yet), `null` once it did not answer. */
export interface PeerState {
  peer: LanPeer;
  hello: LanHello | null | undefined;
}

/** Why a row cannot be opened; the page turns each into a sentence. */
export type Blocked = RowBlocked;

export interface SurfaceRow extends Omit<RowAnswer, "target"> {
  /** The address, when the row opens. */
  href?: string;
}

export interface DeviceGroup extends Omit<RowGroup, "rows"> {
  rows: SurfaceRow[];
}

export interface RowsInput {
  peers: PeerState[];
  /** This device's own engine id, dropped from the list if it ever appears. */
  selfId: string | null;
  /** This device's name, which an older peer is told to update beside. */
  selfName: string;
  /** Whether a drawer for a surface type exists in this build. */
  drawable: (type: string) => boolean;
  origin: string;
}

/** Devices by name; a device not yet answered is left out, so the list does not grow line by line. */
export function surfaceGroups(input: RowsInput): DeviceGroup[] {
  const types = new Set<string>();
  for (const { hello } of input.peers)
    for (const s of hello?.surfaces ?? []) types.add(s.type);
  const groups = surfaceGroupsOf({
    peers: input.peers.map(({ peer, hello }) => ({
      peer,
      asked: hello !== undefined,
      hello: hello ?? null,
    })),
    selfId: input.selfId ?? undefined,
    selfName: input.selfName,
    drawable: [...types].filter(input.drawable),
    versions: {
      api: LAN_API,
      minProto: MIN_ENGINE_PROTOCOL,
      proto: ENGINE_PROTOCOL,
    },
  });
  return groups
    .map((g) => ({
      ...g,
      rows: g.rows.map(
        ({ target, ...row }): SurfaceRow =>
          target
            ? {
                ...row,
                href: lanSurfaceUrl(input.origin, target, target.surface, "k7"),
              }
            : row,
      ),
    }))
    .sort((a, b) => a.device.localeCompare(b.device));
}
