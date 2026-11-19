// PluginScanBar.tsx — the home page's bottom bar while this device's engine scans its plugins.
// Nothing is claimed — the socket only listens. Gone when the scan ends or no engine reports one. See
// ../../docs/projects.md.

import { watchPluginScan } from "@studio/engine-browser/scan-watch";
import type { PluginScanStatus } from "@studio/engine-protocol/decode";
import { useEffect, useState } from "react";
import { engines } from "../api";
import { scanStep } from "../components/StartupBar";
import { localEngineOf } from "../localEngine";

export function PluginScanBar() {
  const [scan, setScan] = useState<PluginScanStatus | null>(null);
  useEffect(() => {
    let stop: (() => void) | null = null;
    let live = true;
    void engines
      .find()
      .then(({ ranked }) => {
        const r = localEngineOf(ranked);
        if (live && r) stop = watchPluginScan(engines.linkUrl(r), setScan);
      })
      .catch(() => {});
    return () => {
      live = false;
      stop?.();
    };
  }, []);
  if (!scan?.scanning) return null;
  const step = scanStep(scan);
  return (
    <div role="status" aria-live="polite" className="text-xs">
      <div
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={step.pct ?? undefined}
        className="h-0.5 w-full overflow-hidden bg-muted"
      >
        <div
          className={
            step.pct === null
              ? "h-full w-1/3 animate-pulse bg-primary"
              : "h-full bg-primary transition-all"
          }
          style={step.pct === null ? undefined : { width: `${step.pct}%` }}
        />
      </div>
      <div className="flex items-center gap-2 px-3 py-1 text-muted-foreground">
        <span className="truncate">{step.label}</span>
        {scan.current && (
          <span className="truncate opacity-70">{scan.current}</span>
        )}
      </div>
    </div>
  );
}
