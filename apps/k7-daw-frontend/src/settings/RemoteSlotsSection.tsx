// RemoteSlotsSection.tsx — the slots of each engine this account owns elsewhere, read and set through the server
// its machine holds a tunnel to. A machine that is not connected says why on its own card. See ../../docs/slots.md.

import { useCallback, useEffect, useState } from "react";
import { m } from "../paraglide/messages.js";
import type { PublicEngine } from "../types";
import { askFootswitch } from "./askFootswitch";
import { useDesktopEngine } from "./desktopEngine";
import { engineSlots, type MachineSlots, ownedEngines } from "./engineSlots";
import { Action, Row, Section } from "./Section";
import { slotDetail } from "./SlotsSection";

function MachineSlotsCard({ engine }: { engine: PublicEngine }) {
  const [state, setState] = useState<MachineSlots | null>(null);
  const [error, setError] = useState<string | null>(null);
  const act = useCallback(async (p: Promise<MachineSlots>) => {
    setError(null);
    try {
      setState(await p);
    } catch (e) {
      setError((e as Error).message);
    }
  }, []);
  useEffect(() => {
    void act(engineSlots.list(engine));
  }, [act, engine]);
  return (
    <Section
      title={m["slots.remoteTitle"]({ host: engine.hostname })}
      hint={m["slots.remoteHint"]()}
    >
      {error && <Row label={error} />}
      {!state && !error && <Row label={m["slots.reading"]()} />}
      {state?.slots.length === 0 && <Row label={m["slots.none"]()} />}
      {state?.slots.map((slot) => (
        <Row key={slot.id} label={slot.name} detail={slotDetail(slot)}>
          {state.enabledSlot === slot.id ? (
            <Action onClick={() => void act(engineSlots.disable(engine))}>
              {m["slots.disable"]()}
            </Action>
          ) : (
            <Action
              onClick={() => void act(engineSlots.enable(engine, slot.id))}
            >
              {m["slots.enable"]()}
            </Action>
          )}
          <Action
            onClick={async () => {
              const cc = await askFootswitch(slot.name, slot.footswitch);
              if (cc !== undefined)
                await act(engineSlots.footswitch(engine, slot.id, cc));
            }}
          >
            {m["slots.footswitch"]()}
          </Action>
          <Action
            danger
            onClick={() => void act(engineSlots.remove(engine, slot.id))}
          >
            {m["slots.remove"]()}
          </Action>
        </Row>
      ))}
    </Section>
  );
}

export function RemoteSlotsSection() {
  const hereId = useDesktopEngine()?.snapshot?.engineId;
  const [owned, setOwned] = useState<PublicEngine[]>([]);
  useEffect(() => {
    let live = true;
    void ownedEngines(hereId).then((list) => live && setOwned(list));
    return () => {
      live = false;
    };
  }, [hereId]);
  return (
    <>
      {owned.map((e) => (
        <MachineSlotsCard key={e.id} engine={e} />
      ))}
    </>
  );
}
