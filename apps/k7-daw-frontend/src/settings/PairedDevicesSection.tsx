// PairedDevicesSection.tsx — the LAN devices paired with this machine's engine, each revocable, and
// the requests waiting for an answer. Only the engine's own machine may manage them: from anywhere
// else the engine refuses, and the section says where to go instead. See ../../docs/pairing.md.

import { EngineClient } from "@studio/engine-browser";
import { bestEngine } from "@studio/engine-browser/choose";
import { isReachable } from "@studio/engine-browser/probe";
import { askConfirm } from "@studio/project-shell/ask";
import { useEffect, useState } from "react";
import { engines } from "../api";
import { PairRequestCard } from "../pairing/PairRequestsPrompt";
import { usePairing } from "../pairing/usePairing";
import { m } from "../paraglide/messages.js";
import { getLocale } from "../paraglide/runtime.js";
import type { RankedEngine } from "../types";
import { Action, Row, Section } from "./Section";

/** The engine this page reached over loopback, if any — the one on this machine — else the one k7 would pick. */
function machineEngine(
  ranked: RankedEngine[],
  chosen: RankedEngine | null,
): RankedEngine | null {
  const local = ranked.find(
    (r) =>
      isReachable(r) &&
      /^wss?:\/\/(127\.0\.0\.1|localhost|\[::1\])[:/]/.test(r.wsUrl),
  );
  return local ?? chosen ?? bestEngine(ranked);
}

const when = (ms: number): string =>
  ms
    ? new Date(ms).toLocaleString(getLocale(), {
        dateStyle: "medium",
        timeStyle: "short",
      })
    : "—";

/** How long the engine may stay silent before the section offers Retry. */
const CONNECT_PATIENCE_MS = 6000;

export function PairedDevicesSection() {
  // `undefined` while probing, null when no engine answers.
  const [client, setClient] = useState<EngineClient | null | undefined>(
    undefined,
  );
  const [connected, setConnected] = useState(false);
  // Retry restarts the probe; `late` says the engine has been silent too long to keep saying "Connecting…".
  const [attempt, setAttempt] = useState(0);
  const [late, setLate] = useState(false);

  // biome-ignore lint/correctness/useExhaustiveDependencies: `attempt` is the Retry trigger, not a value the probe reads.
  useEffect(() => {
    let live = true;
    setClient(undefined);
    setConnected(false);
    setLate(false);
    const lateTimer = setTimeout(() => setLate(true), CONNECT_PATIENCE_MS);
    let engine: EngineClient | null = null;
    let offConn = () => {};
    void engines
      .find()
      .then(({ ranked, chosen }) => {
        const target = live ? machineEngine(ranked, chosen) : null;
        if (!live) return;
        if (!target?.wsUrl) return setClient(null);
        engine = new EngineClient(engines.linkUrl(target));
        offConn = engine.onConnected(setConnected);
        engine.connect();
        setClient(engine);
      })
      .catch(() => live && setClient(null));
    return () => {
      live = false;
      clearTimeout(lateTimer);
      offConn();
      engine?.dispose();
    };
  }, [attempt]);

  const { requests, devices, localOnly } = usePairing(client, connected);

  // A revoked phone has to pair again on this machine, so the tap is confirmed.
  const revoke = async (d: (typeof devices)[number]) => {
    if (!client) return;
    const ok = await askConfirm({
      title: m["pairing.revokeTitle"]({ name: d.name }),
      body: m["pairing.revokeBody"]({ name: d.name }),
      confirmLabel: m["pairing.revoke"](),
      danger: true,
    });
    if (ok) client.unpairDevice(d.deviceId);
  };

  return (
    <Section
      title={m["pairing.settingsTitle"]()}
      hint={m["pairing.settingsHint"]()}
    >
      {client === null ? (
        <Row label={m["pairing.noEngine"]()} />
      ) : localOnly ? (
        <Row label={m["pairing.localOnly"]()} />
      ) : !client || !connected ? (
        late ? (
          <Row label={m["pairing.noAnswer"]()}>
            <Action onClick={() => setAttempt((n) => n + 1)}>
              {m["common.retry"]()}
            </Action>
          </Row>
        ) : (
          <Row label={m["pairing.connecting"]()} />
        )
      ) : (
        <>
          {requests.map((r) => (
            <div key={r.requestId} className="px-4 py-2.5">
              <PairRequestCard engine={client} request={r} />
            </div>
          ))}
          {devices.length === 0 ? (
            <Row label={m["pairing.noDevices"]()} />
          ) : (
            devices.map((d) => (
              <Row
                key={d.deviceId}
                label={d.name}
                detail={m["pairing.deviceDetail"]({
                  paired: when(d.created),
                  seen: when(d.lastSeen),
                })}
              >
                <Action
                  danger
                  label={m["pairing.revokeNamed"]({ name: d.name })}
                  onClick={() => void revoke(d)}
                >
                  {m["pairing.revoke"]()}
                </Action>
              </Row>
            ))
          )}
        </>
      )}
    </Section>
  );
}
