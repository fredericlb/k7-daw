// engineSlots.ts — the slots of an engine this account OWNS elsewhere, set through the server its machine holds a
// tunnel to: the chain compiled there, sent to the machine as a job. This machine's own slots go through its own
// backend instead (desktopEngine.ts). See ../../docs/slots.md.

import { engines, PL8_API } from "../api";
import type { PublicEngine } from "../types";

/** One slot as a machine's supervisor lists it. */
interface SlotRow {
  id: string;
  name: string;
  chain: string;
  savedAt: string;
  /** The MIDI CC whose press bypasses its chain; null for none. */
  footswitch?: number | null;
}

/** A machine's slots and the enabled one, as its supervisor answers every slot job. */
export interface MachineSlots {
  slots: SlotRow[];
  enabledSlot: string | null;
}

/** The engines this account owns at this origin, but not `exceptId` (this machine's). */
export async function ownedEngines(exceptId?: string): Promise<PublicEngine[]> {
  const list = await engines.list().catch(() => [] as PublicEngine[]);
  return list.filter((e) => e.access === "owner" && e.id !== exceptId);
}

/** Where `e`'s slot routes answer from this page: the origin holding its tunnel. */
const base = (e: PublicEngine): string =>
  `${PL8_API}/engine-slots/${encodeURIComponent(e.id)}`;

async function call(
  e: PublicEngine,
  path: string,
  method = "GET",
  json?: unknown,
): Promise<MachineSlots> {
  const r = await fetch(`${base(e)}${path}`, {
    method,
    credentials: "include",
    headers: json ? { "content-type": "application/json" } : undefined,
    body: json ? JSON.stringify(json) : undefined,
  });
  const body = (await r.json().catch(() => ({}))) as Partial<MachineSlots> & {
    error?: string;
  };
  if (!r.ok) throw new Error(body.error ?? `${r.status}`);
  return { slots: body.slots ?? [], enabledSlot: body.enabledSlot ?? null };
}

export const engineSlots = {
  list: (e: PublicEngine) => call(e, "/slots"),
  save: (e: PublicEngine, ref: string) => call(e, "/slots", "POST", { ref }),
  enable: (e: PublicEngine, id: string) =>
    call(e, `/slots/${encodeURIComponent(id)}/enable`, "POST"),
  disable: (e: PublicEngine) => call(e, "/slot/disable", "POST"),
  footswitch: (e: PublicEngine, id: string, cc: number | null) =>
    call(e, `/slots/${encodeURIComponent(id)}/footswitch`, "POST", { cc }),
  remove: (e: PublicEngine, id: string) =>
    call(e, `/slots/${encodeURIComponent(id)}`, "DELETE"),
};
