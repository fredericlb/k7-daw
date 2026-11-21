// holdDrag.ts — touch starts a drag by HOLDING: a finger that stays HOLD_MS arms it, one that moves first scrolls.
// A mouse or a pen arms at once. The one thing a reader would get wrong: a hold released without moving is the long-press menu,
// opened here on release, because the app-wide long press (longPress.ts) is cancelled the moment a hold arms.

import { suppressLongPress, type Timers } from "./longPress";

export const HOLD_MS = 300;
export const HOLD_SLOP_PX = 8;

export interface HoldPoint {
  pointerId: number;
  clientX: number;
  clientY: number;
}

export interface HoldHooks {
  /** The hold elapsed, or a mouse pressed: start the drag. */
  arm: () => void;
  /** A touch released before the hold elapsed, without moving: a tap. */
  tap?: () => void;
  /** An armed hold released where it was pressed, without having dragged: open the menu here. */
  release?: (x: number, y: number) => void;
}

/** One press's state: waiting for the hold, armed, or over. Timers are injected so a test needs no clock. */
export class HoldGesture {
  private state: "waiting" | "armed" | "over" = "waiting";
  private far = false;
  private timer: unknown;

  constructor(
    private readonly origin: HoldPoint,
    private readonly hooks: HoldHooks,
    private readonly timers: Timers,
    private readonly onArmed: () => void = () => {},
  ) {
    this.timer = timers.set(() => {
      if (this.state !== "waiting") return;
      this.state = "armed";
      this.onArmed();
      hooks.arm();
    }, HOLD_MS);
  }

  get armed(): boolean {
    return this.state === "armed";
  }

  get over(): boolean {
    return this.state === "over";
  }

  move(p: HoldPoint): void {
    if (p.pointerId !== this.origin.pointerId || this.state === "over") return;
    const far =
      Math.hypot(
        p.clientX - this.origin.clientX,
        p.clientY - this.origin.clientY,
      ) > HOLD_SLOP_PX;
    if (!far) return;
    if (this.state === "waiting") this.stop();
    else this.far = true;
  }

  up(p: HoldPoint): void {
    if (p.pointerId !== this.origin.pointerId || this.state === "over") return;
    const was = this.state;
    this.stop();
    if (was === "waiting") this.hooks.tap?.();
    else if (!this.far) this.hooks.release?.(p.clientX, p.clientY);
  }

  cancel(): void {
    this.stop();
  }

  private stop(): void {
    this.timers.clear(this.timer);
    this.state = "over";
  }
}

const realTimers: Timers = {
  set: (fn, ms) => window.setTimeout(fn, ms),
  clear: (h) => window.clearTimeout(h as number),
  now: () => window.performance.now(),
};

/**
 * Arm a drag on a press: at once for a mouse or a pen, after the hold for a finger.
 * While armed, the touch cannot scroll (a non-passive `touchmove` is cancelled) and the element reads `touch-action: none`.
 */
export function holdToArm(
  press: React.PointerEvent,
  hooks: HoldHooks,
  el: HTMLElement = press.currentTarget as HTMLElement,
): void {
  if (press.pointerType !== "touch") {
    hooks.arm();
    return;
  }
  const block = (ev: TouchEvent) => {
    if (ev.cancelable) ev.preventDefault();
  };
  const previous = el.style.touchAction;
  const origin = {
    pointerId: press.pointerId,
    clientX: press.clientX,
    clientY: press.clientY,
  };
  const done = () => {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("pointercancel", onCancel);
    window.removeEventListener("touchmove", block);
    el.style.touchAction = previous;
  };
  const gesture = new HoldGesture(origin, hooks, realTimers, () => {
    // The long press is ours now: it must not also open its menu at 500 ms.
    suppressLongPress();
    window.addEventListener("touchmove", block, { passive: false });
    el.style.touchAction = "none";
    navigator.vibrate?.(8);
  });
  const onMove = (ev: PointerEvent) => {
    gesture.move(ev);
    if (gesture.over) done();
  };
  const onUp = (ev: PointerEvent) => {
    if (ev.pointerId !== origin.pointerId) return;
    gesture.up(ev);
    done();
  };
  const onCancel = (ev: PointerEvent) => {
    if (ev.pointerId !== origin.pointerId) return;
    gesture.cancel();
    done();
  };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
  window.addEventListener("pointercancel", onCancel);
}

/** Dispatch a context menu on `el` at a point: what the long press does, for the hold released in place. */
export function openMenuAt(el: Element, x: number, y: number): void {
  el.dispatchEvent(
    new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
      clientX: x,
      clientY: y,
      button: 2,
    }),
  );
}
