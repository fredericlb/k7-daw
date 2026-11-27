// viewportRect.ts — picking a layer in the frame: which layer a click lands on and which of the eight grips it grabbed.
// What a grip does to x/y/w/h is Rust's (`eyes::rect`), reached by a typed export, since it runs on every pointer move. See docs/picture.md.

import { dragRectBy } from "@studio/k7-wasm/eyes";

/** A layer's rect in VIEWPORT px, which is exactly what a picture lane's `frame` states. */
export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** What a press grabbed: the body (a move) or one of the eight edges and corners (a resize). */
export type Grip = "body" | "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";

/** A layer as the picking cares about it — hidden means `mute:`, and a hidden layer is not there to click. */
export interface PickLayer {
  id: string;
  rect: Rect;
  hidden: boolean;
}

/** How far outside an edge still counts as grabbing it, in VIEWPORT px at scale 1 — the caller scales it. */
export const GRIP_TOL = 8;

const inside = (r: Rect, x: number, y: number, pad = 0): boolean =>
  x >= r.x - pad &&
  x <= r.x + r.w + pad &&
  y >= r.y - pad &&
  y <= r.y + r.h + pad;

/** Every layer under the point, FRONT first — `layers` is bottom-first, so the last one drawn leads. */
export function layersAt(
  layers: readonly PickLayer[],
  x: number,
  y: number,
): string[] {
  const out: string[] = [];
  for (let i = layers.length - 1; i >= 0; i--) {
    const l = layers[i];
    if (!l.hidden && inside(l.rect, x, y)) out.push(l.id);
  }
  return out;
}

/** The layer a plain click lands on: the front one, which is the one you can see. */
export const hitLayer = (
  layers: readonly PickLayer[],
  x: number,
  y: number,
): string | null => layersAt(layers, x, y)[0] ?? null;

/** …and the NEXT one down, which is how a layer hidden behind a full-frame one is ever reached. */
export function layerBelow(
  layers: readonly PickLayer[],
  x: number,
  y: number,
  current: string | null,
): string | null {
  const stack = layersAt(layers, x, y);
  if (!stack.length) return null;
  const at = current ? stack.indexOf(current) : -1;
  // Wraps, so cycling comes back to the front rather than stopping at the bottom with nothing to say.
  return stack[(at + 1) % stack.length];
}

/** Which grip a press on the SELECTED layer took, or null when it missed the rect and its border. */
export function gripAt(
  r: Rect,
  x: number,
  y: number,
  tol = GRIP_TOL,
): Grip | null {
  if (!inside(r, x, y, tol)) return null;
  const w = Math.abs(x - r.x) <= tol;
  const e = Math.abs(x - (r.x + r.w)) <= tol;
  const n = Math.abs(y - r.y) <= tol;
  const s = Math.abs(y - (r.y + r.h)) <= tol;
  // Corners before edges: on a small layer the two bands overlap, and a corner is the more specific answer.
  if (n && w) return "nw";
  if (n && e) return "ne";
  if (s && w) return "sw";
  if (s && e) return "se";
  if (n) return "n";
  if (s) return "s";
  if (w) return "w";
  if (e) return "e";
  return inside(r, x, y) ? "body" : null;
}

/** The rect a drag lands on: a grip moves the edges it names and leaves the others where they are, in whole pixels. */
export function dragRect(r: Rect, grip: Grip, dx: number, dy: number): Rect {
  const [x, y, w, h] = dragRectBy([r.x, r.y, r.w, r.h], grip, dx, dy);
  return { x, y, w, h };
}

/** The CSS cursor a grip asks for — one table, so the pointer cannot say a different thing from the drag. */
export const CURSOR: Record<Grip, string> = {
  body: "move",
  n: "ns-resize",
  s: "ns-resize",
  e: "ew-resize",
  w: "ew-resize",
  ne: "nesw-resize",
  sw: "nesw-resize",
  nw: "nwse-resize",
  se: "nwse-resize",
};

/** The eight grips in drawing order, so the overlay and the hit test cannot list different ones. */
export const HANDLES = ["nw", "n", "ne", "e", "se", "s", "sw", "w"] as const;

/** Where a handle's square sits inside the rect, as a 0..1 fraction of each side. */
export const HANDLE_AT: Record<(typeof HANDLES)[number], [number, number]> = {
  nw: [0, 0],
  n: [0.5, 0],
  ne: [1, 0],
  e: [1, 0.5],
  se: [1, 1],
  s: [0.5, 1],
  sw: [0, 1],
  w: [0, 0.5],
};

/** Did the drag change anything? A rect that came back where it started writes nothing. */
export const sameRect = (a: Rect, b: Rect): boolean =>
  a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h;
