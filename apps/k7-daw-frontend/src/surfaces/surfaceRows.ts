// surfaceRows.ts — the Surfaces tab's rows, as data: one group per device, a row per surface, each row either an
// address or the one reason it is not. The filters run in a fixed order and a failed one shows the row disabled,
// never hidden. Pure, so the order is tested without a page. See ../../docs/external.md.

import {
  ENGINE_PROTOCOL,
  MIN_ENGINE_PROTOCOL,
} from "@studio/engine-protocol/version";
import { lanSurfaceUrl } from "@studio/web-protocol/projects";
import { LAN_API, type LanHello, type LanPeer } from "./lanApi";

/** A peer and its hello: `undefined` while the first is asked (not drawn yet), `null` once it did not answer. */
export interface PeerState {
  peer: LanPeer;
  hello: LanHello | null | undefined;
}

/** Why a row cannot be opened; the page turns each into a sentence. */
export type Blocked =
  | { why: "notAnswering" }
  | { why: "notTrusted" }
  | { why: "update"; device: string }
  | { why: "windowOnly" };

export interface SurfaceRow {
  key: string;
  id: string;
  type: string;
  /** The address, when the row opens. */
  href?: string;
  blocked?: Blocked;
  /** The other devices already showing it. */
  alsoOn: string[];
}

export interface DeviceGroup {
  engineId: string;
  device: string;
  /** The project's name, null when the device has none open. */
  project: string | null;
  /** A device-level reason, when no row can be listed at all. */
  blocked?: Blocked;
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

/** The device that must update, or null when the two speak the same: the door's version, then the engine protocol's range. */
export function olderDevice(
  hello: LanHello["device"],
  selfName: string,
): string | null {
  if (hello.api !== LAN_API) return hello.api < LAN_API ? hello.name : selfName;
  if (hello.proto < MIN_ENGINE_PROTOCOL) return hello.name;
  if (hello.proto > ENGINE_PROTOCOL) return selfName;
  return null;
}

/** Devices by name; a device not yet answered is left out, so the list does not grow line by line. */
export function surfaceGroups(input: RowsInput): DeviceGroup[] {
  const groups: DeviceGroup[] = [];
  for (const { peer, hello } of input.peers) {
    if (peer.engineId === input.selfId) continue;
    const name = hello?.device.name || peer.name || peer.host;
    const empty = {
      engineId: peer.engineId,
      device: name,
      project: null,
      rows: [],
    };
    if (!peer.reached) {
      groups.push({ ...empty, blocked: { why: "notAnswering" } });
      continue;
    }
    if (!peer.trusted) {
      groups.push({ ...empty, blocked: { why: "notTrusted" } });
      continue;
    }
    if (hello === null) {
      groups.push({ ...empty, blocked: { why: "notAnswering" } });
      continue;
    }
    // Asked, not answered yet: nothing is drawn for it.
    if (hello === undefined) continue;
    const project = hello.project;
    const updates = olderDevice(hello.device, input.selfName);
    const rows = project
      ? hello.surfaces.map((s): SurfaceRow => {
          const blocked: Blocked | undefined = updates
            ? { why: "update", device: updates }
            : input.drawable(s.type)
              ? undefined
              : { why: "windowOnly" };
          return {
            key: `${peer.engineId}/${s.id}`,
            id: s.id,
            type: s.type,
            href: blocked
              ? undefined
              : lanSurfaceUrl(
                  input.origin,
                  {
                    engineId: peer.engineId,
                    workspace: project.workspace,
                    project: project.path,
                  },
                  s.id,
                  "k7",
                ),
            blocked,
            alsoOn: hello.screens
              .filter((c) => c.surface === s.id)
              .map((c) => c.device),
          };
        })
      : [];
    groups.push({
      engineId: peer.engineId,
      device: name,
      project: project?.label ?? null,
      rows,
    });
  }
  return groups.sort((a, b) => a.device.localeCompare(b.device));
}
