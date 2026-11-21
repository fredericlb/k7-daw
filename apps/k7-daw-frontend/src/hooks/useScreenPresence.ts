// useScreenPresence.ts — this page, a remote, tells the device whose surface it shows (surfaces/presence.ts).
// The screen id is random per mount, so two pages of one device are two screens, as two tablets would be.

import { useEffect, useRef } from "react";
import { lanApi } from "../surfaces/lanApi";
import { startPresence } from "../surfaces/presence";

export function useScreenPresence(
  engineId: string,
  surface: string,
  device: string,
): void {
  const screen = useRef(
    `s${Math.random().toString(36).slice(2, 10)}${Date.now().toString(36)}`,
  ).current;
  useEffect(() => {
    // No device named, no remote: the same hook serves a surface opened on its own machine.
    if (!engineId) return;
    return startPresence({
      beat: () => lanApi.beat(engineId, screen, surface, device),
      bye: () => void lanApi.bye(engineId, screen),
    });
  }, [engineId, screen, surface, device]);
}
