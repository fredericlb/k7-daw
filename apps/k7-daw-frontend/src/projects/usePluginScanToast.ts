// usePluginScanToast.ts — a top toast while this device's engine scans its plugins, gone when the scan ends. Nothing is
// claimed — the socket only listens — and no engine reporting a scan means no toast. See ../../docs/projects.md.

import { watchPluginScan } from "@studio/engine-browser/scan-watch";
import type { PluginScanStatus } from "@studio/engine-protocol/decode";
import {
  dismissToast,
  showToast,
  updateToast,
} from "@studio/project-shell/toast";
import { useEffect } from "react";
import { engines } from "../api";
import { localEngineOf } from "../localEngine";
import { m } from "../paraglide/messages.js";

export function usePluginScanToast(): void {
  useEffect(() => {
    let stop: (() => void) | null = null;
    let live = true;
    let id: number | null = null;
    const onScan = (scan: PluginScanStatus | null) => {
      if (!scan?.scanning) {
        if (id !== null) dismissToast(id);
        id = null;
        return;
      }
      const message = scan.current || m["pluginScan.looking"]();
      const progress =
        scan.total > 0
          ? { value: scan.done, total: scan.total }
          : { value: 0, total: undefined };
      if (id === null)
        id = showToast({
          variant: "neutral",
          placement: "top-centre",
          title: m["pluginScan.title"](),
          message,
          progress,
          duration: 0,
        });
      else updateToast(id, { message, progress });
    };
    void engines
      .find()
      .then(({ ranked }) => {
        const r = localEngineOf(ranked);
        if (live && r) stop = watchPluginScan(engines.linkUrl(r), onScan);
      })
      .catch(() => {});
    return () => {
      live = false;
      stop?.();
      if (id !== null) dismissToast(id);
    };
  }, []);
}
