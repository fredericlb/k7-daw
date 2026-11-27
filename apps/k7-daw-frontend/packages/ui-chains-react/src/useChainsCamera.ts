// useChainsCamera.ts — the camera's frame loop, the wheel and the pinch, and the flights a
// double-click or the minimap asks for. Bound once and writing only refs, so no gesture costs a
// render; the numbers themselves are camera.ts — docs/camera.md.

import type { GraphNode } from "@studio/ui-chains/graph";
import { isMouseWheel, isZoomWheel } from "@studio/ui-timeline/panZoom";
import { usePinch } from "@studio/ui-timeline/pinch";
import {
  type Dispatch,
  type RefObject,
  type SetStateAction,
  useCallback,
  useEffect,
} from "react";
import {
  applyCam,
  approach,
  type Cam,
  clampZoom,
  flyStep,
  frameBox,
  type Lod,
  lodFor,
  sameLod,
  wheelFactor,
  wheelPixels,
  zoomAbout,
} from "./camera";
import type { Motion } from "./nodeMotion";

export function useChainsCamera({
  anchor,
  before,
  cam,
  fit,
  fixedRef,
  flight,
  flightHalf,
  goal,
  holdRowRef,
  hostRef,
  hostSize,
  kick,
  lodRef,
  motionStep,
  motions,
  paintRef,
  pan,
  persistRef,
  placeRef,
  rowRef,
  setLod,
  setPanning,
  settleMotion,
  userMoved,
  worldRef,
}: {
  anchor: RefObject<{ px: number; py: number }>;
  before: RefObject<Cam | null>;
  cam: RefObject<Cam>;
  fit: RefObject<() => boolean>;
  fixedRef: RefObject<boolean>;
  flight: RefObject<Cam | null>;
  flightHalf: RefObject<number | undefined>;
  goal: RefObject<Cam>;
  holdRowRef: RefObject<(c: Cam) => void>;
  hostRef: RefObject<HTMLDivElement | null>;
  hostSize: RefObject<{ w: number; h: number }>;
  kick: RefObject<() => void>;
  lodRef: RefObject<Lod>;
  motionStep: RefObject<(dt: number) => boolean>;
  motions: RefObject<Map<string, Motion>>;
  paintRef: RefObject<() => void>;
  pan: RefObject<{
    sx: number;
    sy: number;
    cx: number;
    cy: number;
    moved: number;
  } | null>;
  persistRef: RefObject<() => void>;
  placeRef: RefObject<(c: Cam) => void>;
  rowRef: RefObject<boolean | undefined>;
  setLod: Dispatch<SetStateAction<Lod>>;
  setPanning: Dispatch<SetStateAction<boolean>>;
  settleMotion: RefObject<() => void>;
  userMoved: RefObject<boolean>;
  worldRef: RefObject<HTMLDivElement | null>;
}) {
  // The camera loop, the wheel and the pinch, bound once and writing only refs — docs/camera.md.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the inputs are refs and setters DomChains hands in, stable for its lifetime.
  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let raf = 0;
    let last = 0;
    const step = (t: number) => {
      // Clamped: a backgrounded tab hands back one enormous dt, which would land the glide at once.
      const dt = last ? Math.min(100, t - last) : 16;
      last = t;
      const c = cam.current;
      const g = goal.current;
      let moving = pan.current != null;
      // The cards, before the camera: `motionStep` rebuilds the cable draws for this frame.
      if (motions.current.size) {
        const still = motionStep.current(dt);
        moving = moving || still;
        if (!still) settleMotion.current();
      }
      const fl = flight.current;
      if (fl) {
        const next = flyStep(
          c,
          fl,
          approach(dt, flightHalf.current),
          host.clientWidth,
          host.clientHeight,
        );
        Object.assign(c, next);
        Object.assign(g, next);
        if (next.k === fl.k && next.x === fl.x && next.y === fl.y)
          flight.current = null;
        else moving = true;
      }
      if (c.k !== g.k) {
        // Within a thousandth of the goal, land on it — an exponential never quite arrives.
        const near = Math.abs(g.k - c.k) <= c.k * 1e-3;
        const k = near ? g.k : c.k + (g.k - c.k) * approach(dt);
        Object.assign(c, zoomAbout(c, k, anchor.current.px, anchor.current.py));
        Object.assign(g, { x: c.x, y: c.y });
        moving = moving || !near;
      }
      // A row never walks off its ends, whichever gesture moved it.
      holdRowRef.current(c);
      holdRowRef.current(g);
      applyCam(worldRef.current, c);
      paintRef.current(); // the canvas is outside `.dgx-world`, so the transform misses it
      const next = lodFor(c.k);
      if (!sameLod(next, lodRef.current)) {
        lodRef.current = next;
        setLod(next);
      }
      if (moving) raf = requestAnimationFrame(step);
      else {
        raf = 0;
        last = 0;
        persistRef.current(); // settled — this is the camera worth remembering
      }
    };
    kick.current = () => {
      if (!raf) {
        last = 0;
        raf = requestAnimationFrame(step);
      }
    };
    // Non-passive: it must preventDefault, and a React onWheel would allocate an event per notch.
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      // A tab column longer than its card scrolls under the wheel, as a list does, instead of moving the camera.
      const col = (e.target as Element | null)?.closest?.(".dgx-tabs");
      if (
        col instanceof HTMLElement &&
        col.scrollHeight > col.clientHeight &&
        !e.ctrlKey
      ) {
        col.scrollTop += wheelPixels(e.deltaY, e.deltaMode, col.clientHeight);
        return;
      }
      if (fixedRef.current) return;
      flight.current = null;
      userMoved.current = true; // either way, the camera is theirs now — stop auto-framing
      if (rowRef.current) {
        // A ROW is read across: the wheel walks along it, whatever axis the device reports.
        const c = cam.current;
        const g = goal.current;
        g.k = c.k;
        c.x = g.x =
          g.x -
          wheelPixels(e.deltaX || e.deltaY, e.deltaMode, host.clientWidth);
      } else if (isZoomWheel(e) || isMouseWheel(e)) {
        const r = host.getBoundingClientRect();
        anchor.current = { px: e.clientX - r.left, py: e.clientY - r.top };
        // Multiplied onto the GOAL, and `e.ctrlKey` marks a PINCH with its own rate — docs/camera.md.
        goal.current.k = clampZoom(
          goal.current.k *
            wheelFactor(e.deltaY, e.deltaMode, host.clientHeight, e.ctrlKey),
        );
      } else {
        // Two fingers on a SURFACE: the camera moves WITH them, 1:1 and with no glide, in screen px.
        const c = cam.current;
        const g = goal.current;
        g.k = c.k; // a slide takes over from any zoom still in flight, as a pointer-down does
        c.x = g.x = g.x - wheelPixels(e.deltaX, e.deltaMode, host.clientWidth);
        c.y = g.y = g.y - wheelPixels(e.deltaY, e.deltaMode, host.clientHeight);
      }
      kick.current();
    };
    host.addEventListener("wheel", wheel, { passive: false });

    // The keys, only while the canvas itself has focus: arrows pan, plus and minus zoom, 0 frames the graph.
    const keys = (e: KeyboardEvent) => {
      if (e.target !== host || fixedRef.current) return;
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      const PAN = 48;
      const dx =
        e.key === "ArrowLeft" ? PAN : e.key === "ArrowRight" ? -PAN : 0;
      const dy = e.key === "ArrowUp" ? PAN : e.key === "ArrowDown" ? -PAN : 0;
      const zoom =
        e.key === "+" || e.key === "="
          ? 1.25
          : e.key === "-" || e.key === "_"
            ? 0.8
            : 1;
      if (e.key === "0") {
        e.preventDefault();
        fit.current();
        return;
      }
      if (!dx && !dy && zoom === 1) return;
      e.preventDefault();
      flight.current = null;
      userMoved.current = true;
      const c = cam.current;
      const g = goal.current;
      if (zoom !== 1 && !rowRef.current) {
        anchor.current = {
          px: host.clientWidth / 2,
          py: host.clientHeight / 2,
        };
        g.k = clampZoom(g.k * zoom);
      } else {
        g.k = c.k;
        c.x = g.x = g.x + dx;
        if (!rowRef.current) c.y = g.y = g.y + dy;
      }
      kick.current();
    };
    host.addEventListener("keydown", keys);

    return () => {
      host.removeEventListener("wheel", wheel);
      host.removeEventListener("keydown", keys);
      if (raf) cancelAnimationFrame(raf);
    };
  }, []);

  // Two fingers where there is no wheel at all, absolute from the camera the gesture started with — docs/camera.md.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the inputs are refs and setters DomChains hands in, stable for its lifetime.
  usePinch<Cam>(
    hostRef,
    {
      onStart: () => {
        if (fixedRef.current) return false;
        flight.current = null;
        pan.current = null; // a pinch is not a pan, whatever the first finger had started
        setPanning(false);
        return { ...cam.current };
      },
      onPinch: (from, ratio, c) => {
        const r = hostRef.current?.getBoundingClientRect();
        if (!r) return;
        userMoved.current = true;
        // From the gesture's OWN starting camera, which is what makes the whole pinch absolute.
        Object.assign(
          cam.current,
          zoomAbout(from, clampZoom(from.k * ratio), c.x - r.left, c.y - r.top),
        );
        Object.assign(goal.current, cam.current);
        kick.current();
      },
    },
    [],
  );

  // --- the double-click camera: onto a node, and back to where it was — docs/camera.md -----
  // biome-ignore lint/correctness/useExhaustiveDependencies: the inputs are refs and setters DomChains hands in, stable for its lifetime.
  const flyTo = useCallback((to: Cam) => {
    userMoved.current = true; // a camera somebody asked for; no resize re-frames it away
    goal.current.k = cam.current.k; // a zoom still gliding gives way, as it does to a press
    flightHalf.current = undefined;
    flight.current = to;
    kick.current();
  }, []);
  // biome-ignore lint/correctness/useExhaustiveDependencies: the inputs are refs and setters DomChains hands in, stable for its lifetime.
  const zoomToNode = useCallback(
    (n: GraphNode) => {
      const host = hostRef.current;
      // A ROW is walked along rather than zoomed into — docs/camera.md.
      if (!host || rowRef.current || fixedRef.current) return;
      // The view before the FIRST node visited, however many are visited after it.
      before.current ??= { ...cam.current };
      flyTo(
        frameBox(
          { x: n.x, y: n.y, w: n.w, h: n.h },
          host.clientWidth,
          host.clientHeight,
        ),
      );
    },
    [flyTo],
  );
  // Only on the BARE canvas: a card, a badge, a socket or a cable is not empty space.
  const canvasDoubleClick = (e: React.MouseEvent) => {
    if (
      (e.target as Element).closest(
        ".dgx-node, .dgx-frame, .dgx-alts, .dgx-engine, .dgx-port, .dgx-hit, .dgx-ins-btn",
      )
    )
      return;
    const back = before.current;
    if (!back) return;
    before.current = null;
    flyTo(back);
  };

  // The minimap: a press puts the pane's centre there, and "Zoom fit" is the first frame again.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the inputs are refs and setters DomChains hands in, stable for its lifetime.
  const centerOn = useCallback((wx: number, wy: number) => {
    const { w, h } = hostSize.current;
    const k = cam.current.k;
    flight.current = null;
    userMoved.current = true;
    placeRef.current({ k, x: w / 2 - wx * k, y: h / 2 - wy * k });
  }, []);
  // biome-ignore lint/correctness/useExhaustiveDependencies: the inputs are refs and setters DomChains hands in, stable for its lifetime.
  const zoomFit = useCallback(() => {
    flight.current = null;
    before.current = null;
    // The camera is the graph's again: a resize re-frames it, as it did before anybody moved it.
    userMoved.current = false;
    fit.current();
  }, []);
  return { canvasDoubleClick, centerOn, zoomFit, zoomToNode };
}
