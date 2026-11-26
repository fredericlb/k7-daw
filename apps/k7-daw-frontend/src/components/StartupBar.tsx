// StartupBar.tsx — what the open project is still waiting for before it is heard: an engine being looked for,
// the project copied to it, applied, warmed up. The project is drawn and editable the whole time; this only says
// what Play is waiting for, as one toast (shell/engineToasts.tsx). See ../../docs/shell.md.

import type {
  EngineLoadPhase,
  EngineScanState,
  EngineSyncState,
} from "@studio/web-protocol";
import { m } from "../paraglide/messages.js";
import { useProgressToast } from "../shell/engineToasts";

/** The engine's plugin scan as one line and a fraction, the same on the home page and under a project. */
function scanStep(scan: { done: number; total: number } | null): {
  label: string;
  pct: number | null;
} {
  if (!scan || scan.total <= 0)
    return { label: m["pluginScan.listing"](), pct: null };
  return {
    label: m["pluginScan.progress"]({
      done: String(scan.done),
      total: String(scan.total),
    }),
    pct: Math.min(100, Math.round((scan.done / scan.total) * 100)),
  };
}

/** The step under way, or null when there is nothing left to wait for. */
export function startupStep({
  searching,
  hasEngine,
  load,
  sync,
  scan = null,
  connecting,
}: {
  searching: boolean;
  hasEngine: boolean;
  load: EngineLoadPhase | null | undefined;
  sync: EngineSyncState | null;
  /** The engine's plugin scan, while a project naming an external plugin waits for it. */
  scan?: EngineScanState | null;
  connecting: boolean;
}): { label: string; pct: number | null } | null {
  if (!hasEngine)
    return searching ? { label: m["startup.searching"](), pct: null } : null;
  if (load === "scanning") return scanStep(scan);
  if (load === "copying") {
    const measured = sync && sync.bytes > 0;
    return {
      label: m["engineLoad.copyingTitle"](),
      pct: measured
        ? Math.min(100, Math.round((sync.bytesDone / sync.bytes) * 100))
        : null,
    };
  }
  if (load === "applying")
    return { label: m["engineLoad.applyingTitle"](), pct: null };
  if (load === "warming")
    return { label: m["engineLoad.warmingTitle"](), pct: null };
  return connecting ? { label: m["startup.connecting"](), pct: null } : null;
}

/** Whether Play must wait for the project: still copied, applied or warmed, or not yet asked (`undefined`) — a Play sent now is dropped. */
export function projectLoading(
  load: EngineLoadPhase | null | undefined,
): boolean {
  return load !== null;
}

/** Draws nothing: its toast follows the step. The plugin scan is the one that sits at the top. */
export function StartupBar(props: Parameters<typeof startupStep>[0]) {
  const step = startupStep(props);
  useProgressToast(step ? { ...step, top: props.load === "scanning" } : null);
  return null;
}
