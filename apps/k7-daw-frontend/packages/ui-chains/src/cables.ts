// cables.ts — the cable DRAWING geometry: smooth a routed polyline into cubic beziers,
// resample it by arc-length for the live-signal wobble, find the insert `+` midpoint,
// and resolve what an arrow automates. Pure math over the laid-out graph (graph.ts
// owns the layout); the signal's strength along the edges is `computeFlow` in `@studio/k7-wasm/chains`.

import { panelWidgets } from "@studio/graph-model/panel";
import { jackRefOp } from "@studio/k7-wasm/graph";
import type { GraphEdge, GraphNode, Pt } from "./graph";
import { columnW, curveOutPoint, menuH, stripWidgetRect } from "./graph";

// A cubic bezier segment (Catmull-Rom → Bezier — see cubicSegments).
export interface Cubic {
  p0: Pt;
  c1: Pt;
  c2: Pt;
  p1: Pt;
}

// The cubics a polyline becomes, split out so the wobble samples the SAME curve. See docs/wobble.md.
export function cubicSegments(pts: Pt[]): Cubic[] {
  if (pts.length < 2) return [];
  // Two points draw straight, expressed as a degenerate cubic so callers have one shape to handle.
  if (pts.length === 2)
    return [{ p0: pts[0], c1: pts[0], c2: pts[1], p1: pts[1] }];
  const out: Cubic[] = [];
  for (let i = 0; i < pts.length - 1; i++) {
    const p0 = pts[Math.max(0, i - 1)];
    const p1 = pts[i];
    const p2 = pts[i + 1];
    const p3 = pts[Math.min(pts.length - 1, i + 2)];
    out.push({
      p0: p1,
      c1: { x: p1.x + (p2.x - p0.x) / 6, y: p1.y + (p2.y - p0.y) / 6 },
      c2: { x: p2.x - (p3.x - p1.x) / 6, y: p2.y - (p3.y - p1.y) / 6 },
      p1: p2,
    });
  }
  return out;
}

export const cubicAt = (c: Cubic, t: number): Pt => {
  const u = 1 - t;
  const a = u * u * u;
  const b = 3 * u * u * t;
  const d = 3 * u * t * t;
  const e = t * t * t;
  return {
    x: a * c.p0.x + b * c.c1.x + d * c.c2.x + e * c.p1.x,
    y: a * c.p0.y + b * c.c1.y + d * c.c2.y + e * c.p1.y,
  };
};

// The curve's derivative: the wobble displaces along its perpendicular, so a ripple stays square.
const cubicTangent = (c: Cubic, t: number): Pt => {
  const u = 1 - t;
  return {
    x:
      3 * u * u * (c.c1.x - c.p0.x) +
      6 * u * t * (c.c2.x - c.c1.x) +
      3 * t * t * (c.p1.x - c.c2.x),
    y:
      3 * u * u * (c.c1.y - c.p0.y) +
      6 * u * t * (c.c2.y - c.c1.y) +
      3 * t * t * (c.p1.y - c.c2.y),
  };
};

// Convert a polyline to a smooth SVG cubic path, so cables curve instead of jointing between waypoints.
export function smoothPath(pts: Pt[]): string {
  if (pts.length < 2) return "";
  if (pts.length === 2)
    return `M ${pts[0].x} ${pts[0].y} L ${pts[1].x} ${pts[1].y}`;
  let d = `M ${pts[0].x} ${pts[0].y}`;
  for (const c of cubicSegments(pts))
    d += ` C ${c.c1.x} ${c.c1.y}, ${c.c2.x} ${c.c2.y}, ${c.p1.x} ${c.p1.y}`;
  return d;
}

// One resampled point on a cable: position, unit normal, and arc-length travelled.
export interface CableSample {
  x: number;
  y: number;
  nx: number;
  ny: number;
  s: number;
}

// A cable's geometry: the base polyline, the samples the wobble displaces, and the arc length.
export interface CableGeom {
  base: Pt[];
  samples: CableSample[];
  len: number;
}

/** Resample a polyline into evenly-spaced points carrying a normal and arc-length, base kept verbatim. */
export function buildCableGeom(
  pts: Pt[],
  step = 8,
  maxSamples = 96,
): CableGeom {
  const base = pts.map((p) => ({ x: p.x, y: p.y }));
  const segs = cubicSegments(base);

  // Walk the CURVE, not the polyline, `s` accumulating real distance so a wavelength stays even.
  const SUB = 16;
  const segLens = segs.map((c) => {
    let l = 0;
    let prev = cubicAt(c, 0);
    for (let i = 1; i <= SUB; i++) {
      const q = cubicAt(c, i / SUB);
      l += Math.hypot(q.x - prev.x, q.y - prev.y);
      prev = q;
    }
    return l;
  });
  const len = segLens.reduce((a, b) => a + b, 0);
  const spacing = Math.max(step, len / maxSamples);

  const samples: CableSample[] = [];
  let s = 0;
  let prev: Pt | null = null;
  for (let i = 0; i < segs.length; i++) {
    const c = segs[i];
    const segLen = segLens[i];
    if (segLen === 0) continue;
    const steps = Math.max(1, Math.round(segLen / spacing));
    for (let j = 0; j < steps; j++) {
      const t = j / steps;
      const pt = cubicAt(c, t);
      const tan = cubicTangent(c, t);
      const m = Math.hypot(tan.x, tan.y) || 1;
      if (prev) s += Math.hypot(pt.x - prev.x, pt.y - prev.y);
      prev = pt;
      samples.push({ x: pt.x, y: pt.y, nx: -tan.y / m, ny: tan.x / m, s });
    }
  }
  // The endpoint closes the curve, or a live cable stops short of what it lands on.
  const last = segs[segs.length - 1];
  if (last) {
    const pt = cubicAt(last, 1);
    const tan = cubicTangent(last, 1);
    const m = Math.hypot(tan.x, tan.y) || 1;
    if (prev) s += Math.hypot(pt.x - prev.x, pt.y - prev.y);
    samples.push({ x: pt.x, y: pt.y, nx: -tan.y / m, ny: tan.x / m, s });
  }
  return { base, samples, len: s };
}

// The midpoint of a cable by arc-length — where the insert `+` sits.
export function midByArc(g: CableGeom): Pt {
  const half = g.len / 2;
  for (const p of g.samples) if (p.s >= half) return { x: p.x, y: p.y };
  return g.base[g.base.length - 1];
}

/** A stretch of time one video lane is on screen, in SECONDS; `to` may be `Infinity`. See @studio/ui-eyes. */
export interface LaneSpan {
  from: number;
  to: number;
}

/** Which VIDEO lanes carry a picture at `t`, half-open as `activeAt` is. See docs/graphBuild.md. */
export function liveVideoLanes(
  spans: Readonly<Record<string, readonly LaneSpan[]>>,
  t: number,
): string[] {
  const ids: string[] = [];
  for (const id in spans)
    if (spans[id].some((s) => t >= s.from && t < s.to)) ids.push(id);
  return ids;
}

/** The chains.k7x name a JACK click has in hand, plus `#port`; a box answers with the MEMBER behind the plug (the `chains` crate's rule). */
export const jackRef = (n: GraphNode, port?: string): string =>
  jackRefOp(n, port);

// Where an automation arrow lands: the target widget's border, or the module's left edge.
export interface AutoTarget {
  id: string;
  from: Pt;
  to: Pt;
  param: string;
  onWidget: boolean;
  /** The module automated, which unbinding names — absent on a target that is not a module's card. */
  module?: string;
}

export function autoTarget(
  edge: GraphEdge,
  byId: Map<string, GraphNode>,
): AutoTarget | null {
  const src = byId.get(edge.source);
  const tgt = byId.get(edge.target);
  const a = edge.automation;
  if (!src || !tgt || !a) return null;
  // Out of the curve's own socket, into the ring round the knob it drives.
  const from = curveOutPoint(src);
  return {
    id: edge.id,
    from,
    to: a.widget
      ? (widgetBorderPoint(tgt, a.param, from) ?? {
          x: tgt.x + a.widget.x,
          y: tgt.y + a.widget.y,
        })
      : { x: tgt.x, y: tgt.y + tgt.h / 2 },
    param: a.param,
    onWidget: !!a.widget,
    ...(tgt.module ? { module: tgt.module } : {}),
  };
}

/** Where the line from `from` to the widget's centre crosses the box its glow is drawn on — the cable stops at the ring, not inside it. */
function widgetBorderPoint(
  n: GraphNode,
  param: string,
  from: Pt,
): Pt | undefined {
  const w = panelWidgets(n.ui).find(([, v]) => v.bind === param)?.[1];
  if (!w) return undefined;
  const r = stripWidgetRect(n, w, columnW(n.ui), menuH(n.ui));
  const c = { x: n.x + r.x + r.w / 2, y: n.y + r.y + r.h / 2 };
  const dx = from.x - c.x;
  const dy = from.y - c.y;
  // The fraction of the way out at which the line meets a side: the nearer of the two it could cross.
  const t = Math.min(
    dx ? r.w / 2 / Math.abs(dx) : Number.POSITIVE_INFINITY,
    dy ? r.h / 2 / Math.abs(dy) : Number.POSITIVE_INFINITY,
  );
  // A socket inside the box itself has no border to stop at: the centre it is.
  return t >= 1 ? c : { x: c.x + dx * t, y: c.y + dy * t };
}
