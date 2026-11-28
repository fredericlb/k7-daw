// hold.ts — on a touch screen a finger HOLDS before it drags: one that stays HOLD_MS arms the drag, one that moves first does nothing.
// A mouse or a pen arms at once. The one thing a reader would get wrong: a hold released without moving is the long-press menu,
// opened here on release, because the app-wide long press is cancelled the moment a hold arms
// (apps/k7-daw-frontend/docs/timeline.md, "A finger holds, then drags").

import type { DragHandlers } from "@studio/ui/lib/pointer";
import { holdTwoFingers } from "./twoFingers";

export const HOLD_MS = 300;
export const HOLD_SLOP_PX = 8;

export interface Timers {
  set: (fn: () => void, ms: number) => unknown;
  clear: (handle: unknown) => void;
  now: () => number;
}

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
  /** An armed hold released where it was pressed, without having dragged; holdToArm opens the menu there when left out. */
  release?: (x: number, y: number) => void;
}

let onArmed: () => void = () => {};

/** The app's long press registers what a hold does to it when it arms (cancel its own 500 ms menu). */
export function setHoldArmedHook(fn: () => void): void {
  onArmed = fn;
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
    private readonly armedHook: () => void = () => {},
  ) {
    this.timer = timers.set(() => {
      if (this.state !== "waiting") return;
      this.state = "armed";
      this.armedHook();
      hooks.arm();
    }, HOLD_MS);
  }

  get armed(): boolean {
    return this.state === "armed";
  }

  get waiting(): boolean {
    return this.state === "waiting";
  }

  get over(): boolean {
    return this.state === "over";
  }

  /** Whether an armed hold has travelled past the slop: a drag, not a press held in place. */
  get travelled(): boolean {
    return this.far;
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

  /** Another finger landed: a hold still waiting gives way to the two-finger pan, an armed one keeps its drag. */
  otherDown(): void {
    if (this.state === "waiting") this.stop();
  }

  cancel(): void {
    this.stop();
  }

  private stop(): void {
    this.timers.clear(this.timer);
    this.state = "over";
  }
}

/** A drag begun on the press, held back on a finger until the hold arms: moved first it is cancelled, a tap ends it, held in place it asks for the menu. */
export class HoldFollow {
  private readonly g: HoldGesture;
  private done = false;

  constructor(
    origin: HoldPoint,
    private readonly h: DragHandlers,
    timers: Timers,
    private readonly menu: (x: number, y: number) => void,
    armedHook: () => void = () => {},
  ) {
    this.g = new HoldGesture(origin, { arm: () => {} }, timers, armedHook);
  }

  get over(): boolean {
    return this.done;
  }

  move(ev: PointerEvent): void {
    if (this.done) return;
    this.g.move(ev);
    if (this.g.armed) this.h.move?.(ev);
    else if (this.g.over) this.finish();
  }

  end(ev: PointerEvent): void {
    if (this.done) return;
    const tap = this.g.waiting;
    const dragged = this.g.armed && this.g.travelled;
    this.g.cancel();
    if (tap || dragged) {
      this.done = true;
      this.h.end?.(ev);
      return;
    }
    this.finish();
    this.menu(ev.clientX, ev.clientY);
  }

  otherDown(): void {
    if (this.done) return;
    this.g.otherDown();
    if (this.g.over) this.finish();
  }

  cancel(): void {
    this.finish();
  }

  private finish(): void {
    if (this.done) return;
    this.done = true;
    this.g.cancel();
    this.h.cancel?.();
  }
}

const realTimers: Timers = {
  set: (fn, ms) => window.setTimeout(fn, ms),
  clear: (h) => window.clearTimeout(h as number),
  now: () => window.performance.now(),
};

/** What every hold does as it arms: the long press is ours now, two fingers may not pan under the drag, and the device ticks. */
function armTouch(): () => void {
  onArmed();
  navigator.vibrate?.(8);
  return holdTwoFingers();
}

/** Watch for another finger while a press waits; the return stops watching. */
function onOtherFinger(id: number, fn: () => void): () => void {
  const down = (ev: PointerEvent) => {
    if (ev.pointerId !== id && ev.pointerType === "touch") fn();
  };
  window.addEventListener("pointerdown", down, true);
  return () => window.removeEventListener("pointerdown", down, true);
}

const pointOf = (e: HoldPoint): HoldPoint => ({
  pointerId: e.pointerId,
  clientX: e.clientX,
  clientY: e.clientY,
});

/** Arm a drag on a press: at once for a mouse or a pen, after the hold for a finger, which then cannot scroll anything. */
export function holdToArm(
  press: React.PointerEvent,
  hooks: HoldHooks,
  el: HTMLElement = press.currentTarget as HTMLElement,
): void {
  if (press.pointerType !== "touch") {
    hooks.arm();
    return;
  }
  // A second finger is the start of a pan, never a hold of its own.
  if (!press.isPrimary) return;
  const block = (ev: TouchEvent) => {
    if (ev.cancelable) ev.preventDefault();
  };
  const previous = el.style.touchAction;
  const origin = pointOf(press);
  let release = () => {};
  const done = () => {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("pointercancel", onCancel);
    window.removeEventListener("touchmove", block);
    unwatch();
    release();
    el.style.touchAction = previous;
  };
  // The hold took the app-wide long press, so it gives the menu back: on the element the finger pressed, its own handler first.
  const pressed = press.target instanceof Element ? press.target : el;
  const withMenu: HoldHooks = {
    ...hooks,
    release: hooks.release ?? ((x, y) => openMenuAt(pressed, x, y)),
  };
  const gesture = new HoldGesture(origin, withMenu, realTimers, () => {
    release = armTouch();
    window.addEventListener("touchmove", block, { passive: false });
    el.style.touchAction = "none";
  });
  const unwatch = onOtherFinger(origin.pointerId, () => {
    gesture.otherDown();
    if (gesture.over) done();
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

/** A drag's handlers, held back on a finger until the hold arms (HoldFollow); a mouse's and a pen's pass through unchanged. */
export function holdFollow(
  press: React.PointerEvent,
  h: DragHandlers,
): DragHandlers {
  if (press.pointerType !== "touch") return h;
  // A second finger is the start of a pan: whatever its press began is taken back at once.
  if (!press.isPrimary) {
    h.cancel?.();
    return {};
  }
  const pressed =
    press.target instanceof Element ? press.target : document.body;
  let release = () => {};
  const f = new HoldFollow(
    pointOf(press),
    h,
    realTimers,
    (x, y) => openMenuAt(pressed, x, y),
    () => {
      release = armTouch();
    },
  );
  const unwatch = onOtherFinger(press.pointerId, () => {
    f.otherDown();
    settle();
  });
  const settle = () => {
    if (!f.over) return;
    unwatch();
    release();
  };
  return {
    move: (ev) => {
      f.move(ev);
      settle();
    },
    end: (ev) => {
      f.end(ev);
      settle();
    },
    cancel: () => {
      f.cancel();
      settle();
    },
  };
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
