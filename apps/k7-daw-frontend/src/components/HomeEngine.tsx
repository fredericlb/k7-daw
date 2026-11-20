// HomeEngine.tsx — the engine slot of the home pages, above the sidebar's user menu: this device's
// engine as one probe found it, with its figures. There is no session here, so the dot says only
// whether the engine answered. See ../../docs/shell.md.

import { Badge } from "@studio/ui/badge";
import { formatPerformance } from "@studio/web-protocol/performance";
import { formatLatency } from "@studio/web-protocol/quality";
import { useCallback, useEffect, useState } from "react";
import { engines } from "../api";
import { localEngineOf } from "../localEngine";
import { m } from "../paraglide/messages.js";
import type { RankedEngine } from "../types";
import { useIsMobile } from "../useIsMobile";
import { BarEngine } from "./BarEngine";

// One probe shared by every home page for a little while: the sidebar remounts on each route.
const PROBE_TTL_MS = 30_000;
let probe: {
  at: number;
  found: ReturnType<typeof engines.find>;
} | null = null;
const find = (fresh = false) => {
  if (fresh || !probe || Date.now() - probe.at > PROBE_TTL_MS)
    probe = { at: Date.now(), found: engines.find() };
  return probe.found;
};

/** The home pages' engine: this device's, with its probed figures. */
export function HomeEngine() {
  const isMobile = useIsMobile();
  const [current, setCurrent] = useState<RankedEngine | null>(null);
  const [searching, setSearching] = useState(true);

  const load = useCallback(async (fresh = false) => {
    const { ranked } = await find(fresh);
    setCurrent(localEngineOf(ranked));
  }, []);

  useEffect(() => {
    void load()
      .catch(() => {})
      .finally(() => setSearching(false));
  }, [load]);

  const stats = current ? (
    <>
      {current.latency ? (
        <Badge variant="default" title={current.latency.quality}>
          {formatLatency(current.latency)}
        </Badge>
      ) : (
        <Badge variant="amber">{m["engines.unreachable"]()}</Badge>
      )}
      {current.engine.performance && (
        <Badge variant="default">
          {formatPerformance(current.engine.performance)}
        </Badge>
      )}
    </>
  ) : null;

  return (
    <BarEngine
      current={current}
      searching={searching}
      stats={stats}
      // No session here: the probe's answer is the state, an engine that did not answer reads as down.
      status={
        current
          ? current.latency
            ? "up"
            : "down"
          : searching
            ? "searching"
            : "none"
      }
      // Measured when opened: the engine may have been started since the page drew.
      onOpen={() => void load(true).catch(() => {})}
      toolSize={isMobile ? "icon" : "icon-xs"}
      align="end"
    />
  );
}
