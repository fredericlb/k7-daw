// engineToasts.tsx — what the engine is doing, said as toasts: a step under way as a neutral toast with a
// bar, and a lost link as an error with Retry. The header holds no engine state; nothing else says it.
import {
  dismissToast,
  showToast,
  type ToastPlacement,
  updateToast,
} from "@studio/project-shell/toast";
import { Unplug } from "@studio/ui/icons";
import { Spinner } from "@studio/ui/spinner";
import { useEffect, useRef } from "react";
import { m } from "../paraglide/messages.js";

export interface ProgressStep {
  label: string;
  /** 0 to 100, or null when the step cannot say how far it is. */
  pct: number | null;
  /** The plugin scan sits at the top, centred, above everything. */
  top?: boolean;
}

/** One toast following `step`: shown while there is one, updated as it moves, gone when it ends. */
export function useProgressToast(step: ProgressStep | null): void {
  const live = useRef<{ id: number; placement: ToastPlacement } | null>(null);
  const placement: ToastPlacement = step?.top ? "top-centre" : "bottom-right";
  const label = step?.label;
  const pct = step?.pct ?? null;
  const has = step !== null;
  useEffect(() => {
    const cur = live.current;
    if (!has || label === undefined) {
      if (cur) dismissToast(cur.id);
      live.current = null;
      return;
    }
    const progress = pct === null ? undefined : { value: pct };
    if (cur && cur.placement === placement) {
      updateToast(cur.id, { message: label, progress });
      return;
    }
    if (cur) dismissToast(cur.id);
    const id = showToast({
      message: label,
      variant: "neutral",
      placement,
      duration: 0,
      progress,
      icon: progress ? undefined : <Spinner />,
    });
    live.current = { id, placement };
  }, [has, label, pct, placement]);
  useEffect(
    () => () => {
      if (live.current) dismissToast(live.current.id);
      live.current = null;
    },
    [],
  );
}

/** How long the link must stay down before it is said — a reconnect inside it is a blip. */
const LOST_DELAY_MS = 2000;

/** The link down for more than a blip: an error toast with Retry, gone when the link is back. */
export function useEngineLostToast(
  connected: boolean,
  onRetry: () => void,
): void {
  const retry = useRef(onRetry);
  retry.current = onRetry;
  useEffect(() => {
    if (connected) return;
    let id: number | null = null;
    const t = setTimeout(() => {
      id = showToast({
        message: m["engine.lostStrip"](),
        variant: "error",
        icon: <Unplug />,
        action: {
          label: m["engine.retryNow"](),
          onClick: () => retry.current(),
        },
      });
    }, LOST_DELAY_MS);
    return () => {
      clearTimeout(t);
      if (id !== null) dismissToast(id);
    };
  }, [connected]);
}
