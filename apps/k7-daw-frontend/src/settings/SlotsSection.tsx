// SlotsSection.tsx — the engine page's slots: the frozen chains this machine keeps compiled, at most one ENABLED,
// played by the engine with nobody attached until a project opens on it. A chain is saved into a slot from the
// Frozen chains page; here it is enabled, disabled or removed. See ../../docs/slots.md.

import { formatDate } from "@studio/recorder/format";
import { m } from "../paraglide/messages.js";
import { getLocale } from "../paraglide/runtime.js";
import { askFootswitch } from "./askFootswitch";
import { type DesktopEngineSnapshot, engineCommand } from "./desktopEngine";
import { Action, Row, Section } from "./Section";

/** A slot's line under its name: its chain, when it was saved, and its footswitch when it has one. */
export function slotDetail(slot: {
  chain: string;
  savedAt: string;
  footswitch?: number | null;
}): string {
  const saved = m["slots.detail"]({
    chain: slot.chain,
    when: formatDate(slot.savedAt, "date", getLocale()),
  });
  return slot.footswitch == null
    ? saved
    : `${saved} · ${m["slots.footswitchIs"]({ cc: String(slot.footswitch) })}`;
}

export function SlotsSection({ s }: { s: DesktopEngineSnapshot | null }) {
  const slots = s?.slots ?? [];
  const enabled = s?.enabledSlot ?? null;
  return (
    <Section title={m["slots.title"]()} hint={m["slots.hint"]()}>
      {slots.length === 0 && <Row label={m["slots.none"]()} />}
      {slots.map((slot) => (
        <Row key={slot.id} label={slot.name} detail={slotDetail(slot)}>
          {enabled === slot.id ? (
            <Action onClick={() => engineCommand("slot.disable")}>
              {m["slots.disable"]()}
            </Action>
          ) : (
            <Action onClick={() => engineCommand("slot.enable", slot.id)}>
              {m["slots.enable"]()}
            </Action>
          )}
          <Action
            onClick={async () => {
              const cc = await askFootswitch(slot.name, slot.footswitch);
              if (cc !== undefined)
                engineCommand(
                  "slot.footswitch",
                  cc === null ? slot.id : `${slot.id}\u001f${cc}`,
                );
            }}
          >
            {m["slots.footswitch"]()}
          </Action>
          <Action danger onClick={() => engineCommand("slot.remove", slot.id)}>
            {m["slots.remove"]()}
          </Action>
        </Row>
      ))}
    </Section>
  );
}
