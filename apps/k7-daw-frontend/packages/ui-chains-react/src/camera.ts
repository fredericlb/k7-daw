// The graph's camera, as arithmetic: a wheel notch to a zoom goal, a zoom that leaves the point
// under the cursor where it is, the glide that chases the goal, and the little the DRAWING needs
// back from all that. useChainsCamera.ts owns the frame loop; this owns the numbers,
// which is the half worth pinning down — see docs/camera.md.

import { wheelFactor as pinchFactor } from "@studio/ui-timeline/panZoom";

export interface Cam {
  x: number;
  y: number;
  k: number;
}

// Carried over verbatim from the WebGL renderer, so the feel is identical.
export const MIN_ZOOM = 0.2;
export const MAX_ZOOM = 4;
const WIDGET_MIN_SCALE = 0.6; // below this the widget grid is dropped (unreadable anyway)
const LABEL_MIN_SCALE = 0.4; // …and below this the text goes too

// A line, in pixels, for the browsers that measure a wheel in lines rather than pixels.
const LINE_PX = 16;
// Half the remaining distance to the goal, covered in this long — in TIME, so a 144 Hz screen glides no sooner.
export const ZOOM_HALF_LIFE = 45;

export const clampZoom = (k: number): number =>
  Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, k));

/** One wheel delta in PIXELS: `deltaMode` may report lines or pages, a 30× difference — docs/camera.md. */
export function wheelPixels(
  delta: number,
  deltaMode: number,
  viewportPx: number,
): number {
  if (deltaMode === 1) return delta * LINE_PX;
  if (deltaMode === 2) return delta * viewportPx;
  return delta;
}

/** A mouse notch's zoom per pixel of delta: ~10% a detent. A pinch has its own rate, the panes' (`wheelFactor` of panZoom) — docs/camera.md. */
const NOTCH_RATE = 0.001;

/** One wheel event as a zoom FACTOR: normalise the units here, then a notch scales by its delta and a pinch takes the panes' rate — docs/camera.md. */
export function wheelFactor(
  deltaY: number,
  deltaMode: number,
  viewportPx: number,
  pinch = false,
): number {
  const px = wheelPixels(deltaY, deltaMode, viewportPx);
  return pinch ? pinchFactor(px, true) : Math.exp(-px * NOTCH_RATE);
}

/** Where a frame's content starts across the pane: centred, or hard LEFT when it spills — docs/camera.md. */
export function frameOriginX(
  paneW: number,
  contentW: number,
  pad: number,
  spills: boolean,
): number {
  return spills ? (paneW * (1 - pad)) / 2 : (paneW - contentW) / 2;
}

// How far a row may be walked past its last card: the end of the lane, and a little air after it.
export const ROW_TAIL = 30;

/** A row's camera x: its first card no further right than the left edge, its last no further left than `tail` from the right — docs/camera.md. */
export function clampRowX(
  x: number,
  k: number,
  left: number,
  right: number,
  paneW: number,
  margin: number,
  tail = ROW_TAIL,
): number {
  const first = margin - left * k;
  const last = paneW - tail - right * k;
  return Math.max(Math.min(x, first), Math.min(last, first));
}

/** Zoom to `k` keeping the world point under (px, py) under it, on EVERY frame of a glide — docs/camera.md. */
export function zoomAbout(cam: Cam, k: number, px: number, py: number): Cam {
  const f = k / cam.k;
  return { k, x: px - (px - cam.x) * f, y: py - (py - cam.y) * f };
}

// The fraction of the remaining distance to cover in `dt` ms — two half-lives always halve twice.
export function approach(dt: number, halfLife = ZOOM_HALF_LIFE): number {
  return 1 - 2 ** (-Math.max(0, dt) / halfLife);
}

/** Everything the drawing takes from the camera, and all of it — x and y appear nowhere, docs/camera.md. */
export interface Lod {
  widgets: boolean;
  labels: boolean;
  boost: number;
}

export function lodFor(k: number): Lod {
  return {
    widgets: k >= WIDGET_MIN_SCALE,
    labels: k >= LABEL_MIN_SCALE,
    // Cables thicken as the camera pulls back so their width holds on screen, rounded to 5% steps.
    boost: Math.round(Math.min(3, Math.max(1, WIDGET_MIN_SCALE / k)) * 20) / 20,
  };
}

export const sameLod = (a: Lod, b: Lod): boolean =>
  a.widgets === b.widgets && a.labels === b.labels && a.boost === b.boost;

// The one place the camera becomes a transform, written onto the element rather than rendered.
export function applyCam(el: HTMLElement | null, c: Cam): void {
  if (!el) return;
  // The world is laid out at its settled zoom (settleCam); the transform carries only what the gesture adds.
  const z = Number(el.style.zoom) || 1;
  el.style.transform = `translate(${c.x / z}px,${c.y / z}px) scale(${c.k / z})`;
  // The zoom for CSS too, which caps what it draws on SCREEN (chainsCss.ts); set only when it moves, a pan costing no restyle.
  const k = String(c.k);
  if (el.style.getPropertyValue("--dgx-k") !== k)
    el.style.setProperty("--dgx-k", k);
}

/** Lays the world out at the camera's zoom once it settles: CSS zoom paints text crisp, a scale stretches a 1x picture (docs/camera.md). */
export function settleCam(el: HTMLElement | null, c: Cam): void {
  if (!el) return;
  const k = String(c.k);
  if (el.style.zoom !== k) el.style.zoom = k;
  applyCam(el, c);
}

/** How much of the pane a node framed by a double-click fills, and the zoom it never goes past. */
export const FRAME_FILL = 0.7;
export const FRAME_MAX_ZOOM = 2;

/** The camera that centres the box (world units) in a `w` × `h` pane, zoomed to fill it — docs/camera.md. */
export function frameBox(
  box: { x: number; y: number; w: number; h: number },
  w: number,
  h: number,
): Cam {
  const k = clampZoom(
    Math.min(
      FRAME_MAX_ZOOM,
      (w * FRAME_FILL) / Math.max(1, box.w),
      (h * FRAME_FILL) / Math.max(1, box.h),
    ),
  );
  return {
    k,
    x: w / 2 - (box.x + box.w / 2) * k,
    y: h / 2 - (box.y + box.h / 2) * k,
  };
}

/** One frame of a FLIGHT to `to`: the zoom and the world point at the pane's centre both close in by `f`. */
export function flyStep(
  from: Cam,
  to: Cam,
  f: number,
  w: number,
  h: number,
): Cam {
  // The CENTRE moves in world units rather than x and y in screen px, so the target stays in view throughout.
  const cx = (w / 2 - from.x) / from.k;
  const cy = (h / 2 - from.y) / from.k;
  const tx = (w / 2 - to.x) / to.k;
  const ty = (h / 2 - to.y) / to.k;
  const k = from.k + (to.k - from.k) * f;
  const x = cx + (tx - cx) * f;
  const y = cy + (ty - cy) * f;
  const next = { k, x: w / 2 - x * k, y: h / 2 - y * k };
  // Within a thousandth of the zoom and half a pixel of the place, land on it — an exponential never arrives.
  const landed =
    Math.abs(to.k - k) <= k * 1e-3 &&
    Math.abs(to.x - next.x) < 0.5 &&
    Math.abs(to.y - next.y) < 0.5;
  return landed ? { ...to } : next;
}
