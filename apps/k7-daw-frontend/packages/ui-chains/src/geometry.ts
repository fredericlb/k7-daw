// geometry.ts — the chains graph's DRAWING geometry: where a socket and a widget sit, and the cables between sockets.
// The sizes a build decides with are Rust's (the `chains` crate), read here through the generated constants and `nodeSize`.

import type { PageModuleUI, PageWidget } from "@studio/graph-model";
import { hasTabs } from "@studio/graph-model/panel";
import { nodeSizeOp } from "@studio/k7-wasm/graph";
import type { GraphEdge, GraphNode, Pt } from "./graphNode";
import { hangPoints } from "./hang";

export {
  CELL,
  CLIP_LIST_MAX,
  COL_GAP,
  FOOTER_H,
  MAIN_OUT,
  MIX_OUT,
  NODE_H,
  NODE_MIN_W,
  NODE_W,
  RENDER_OUT,
  ROW_GAP,
  TAB_COL_MAX_ROWS,
  TAB_COL_W,
  TAB_H,
  TAB_MENU_H,
  UI_CONTENT_TOP,
  UI_EDGE,
  UI_HEADER,
  UI_PAD,
  UI_TOP_GAP,
  VIDEO_OUT,
} from "@studio/k7-wasm/consts";

import {
  CELL,
  COL_GAP,
  ROW_GAP,
  TAB_COL_W,
  TAB_MENU_H,
  UI_CONTENT_TOP,
  UI_PAD,
} from "@studio/k7-wasm/consts";

/** How far outside a column a side cable's first lane runs, and how far apart two lanes are. */
const SIDE_OFF = 8;
const SIDE_STEP = 6;
/** How far in from the card's corner a side cable leaves and arrives, so it reads as the card's. */
const SIDE_INSET = 10;

/** A side cable: out of the source's right edge, down a lane outside the column, into the target's. */
function sidePath(src: GraphNode, tgt: GraphNode, lane: number): Pt[] {
  const x =
    Math.max(src.x + src.w, tgt.x + tgt.w) + SIDE_OFF + lane * SIDE_STEP;
  const from = { x: src.x + src.w, y: src.y + src.h - SIDE_INSET };
  const to = { x: tgt.x + tgt.w, y: tgt.y + SIDE_INSET };
  const turn = Math.sign(to.y - from.y) * 4;
  // Points either side of each corner, so the smoothing keeps the run down the lane straight.
  return [
    from,
    { x: x - 2, y: from.y },
    { x, y: from.y + turn },
    { x, y: to.y - turn },
    { x: x - 2, y: to.y },
    to,
  ];
}

/** How far down a curve card's LEFT edge the event that fires it plugs in. */
export const EVENT_IN_AT = 0.5;

/** A port's plate, here rather than in the stylesheet so the cable's end and the CSS are one number. */
export const PORT_PLATE = 13;

/** Where a curve card's automation leaves: the middle of its RIGHT edge, toward the node it sits left of. */
export const curveOutPoint = (n: GraphNode): Pt => ({
  x: n.x + n.w + PORT_PLATE / 2,
  y: n.y + n.h / 2,
});

/** An EVENT socket's centre — its own function because it spreads down the SIDE edges, not the flow's. */
export function eventPoint(
  n: GraphNode,
  side: "in" | "out",
  index: number,
): Pt {
  // A curve's event socket is on its LEFT edge (docs/graphBuild.md)…
  if (side === "in")
    return { x: n.x - PORT_PLATE / 2, y: n.y + n.h * EVENT_IN_AT };
  // …and what a module announces leaves from ITS left edge, aligned at the bottom, one plug above the next.
  const count = Math.max(1, n.events?.length ?? 1);
  const i = Math.min(Math.max(0, index), count - 1);
  return { x: n.x - PORT_PLATE / 2, y: n.y + n.h - eventOutBottom(i) };
}

/** How far UP a module's left edge its `i`th event plug's centre sits, in px from the bottom corner, one above the next. */
export const eventOutBottom = (i: number): number => 14 + 22 * i;

/** A socket's centre in graph space, spread evenly down the node's edge as the plate's CSS is. */
export function jackPoint(n: GraphNode, side: "in" | "out", index: number): Pt {
  const count = Math.max(1, (n.jacks?.[side] ?? []).length);
  const i = Math.min(Math.max(0, index), count - 1);
  // A row read ACROSS meets its cables on the sides: out of the right edge, into the left.
  if (n.flow === "across")
    return {
      x: side === "in" ? n.x - PORT_PLATE / 2 : n.x + n.w + PORT_PLATE / 2,
      y: n.y + (n.h * (i + 1)) / (count + 1),
    };
  // Otherwise signal runs down: inputs on the TOP edge, outputs on the bottom, spread across the width.
  return {
    x: n.x + (n.w * (i + 1)) / (count + 1),
    y: side === "in" ? n.y - PORT_PLATE / 2 : n.y + n.h + PORT_PLATE / 2,
  };
}

/** A cable's polyline: it HANGS socket to socket, the layout's waypoints dropped — docs/hang.md, docs/graphBuild.md. */
export function cablePath(
  e: GraphEdge,
  src: GraphNode | undefined,
  tgt: GraphNode | undefined,
): Pt[] {
  const pts = e.points;
  if (pts.length < 2) return pts;
  // A cable that skips cards of its stack column goes round them, on the column's right side.
  if (e.side !== undefined && src && tgt) return sidePath(src, tgt, e.side);
  // An EVENT cable meets the side edges, and its two ends are indexed differently — its own branch.
  if (e.signal === "event" && src && tgt) {
    const i = Math.max(0, src.events?.indexOf(e.sourcePort ?? "") ?? 0);
    return hangPoints(
      eventPoint(src, "out", i),
      eventPoint(tgt, "in", 0),
      true, // always sideways: both ends are on a left or right edge
    );
  }
  // A bare cable leaves the FIRST jack, except off a lane where it leaves the first OCCUPIED one.
  const fromIndex = e.sourcePort
    ? (src?.jacks?.out.findIndex((j) => j.name === e.sourcePort) ?? -1)
    : src?.kind === "track"
      ? (src.jacks?.out.findIndex((j) => !j.free) ?? -1)
      : 0;
  const from = src
    ? jackPoint(src, "out", fromIndex < 0 ? 0 : fromIndex)
    : pts[0];
  // A bare cable takes the socket its RANK earned it; `jackPoint` clamps, so a node drawing one is unharmed.
  const toIndex = e.targetPort
    ? (tgt?.jacks?.in.findIndex((j) => j.name === e.targetPort) ?? -1)
    : (e.targetSlot ?? 0);
  const to = tgt
    ? jackPoint(tgt, "in", toIndex < 0 ? 0 : toIndex)
    : pts[pts.length - 1];
  return hangPoints(from, to, src?.flow === "across" || tgt?.flow === "across");
}

/** The id an assembly's card has, in the family of the `mod:` / `trk:` / `snk:` ids the build mints. */
export const assemblyId = (name: string): string => `asm:${name}`;

/** What an OPEN box adds around the layout of its members: a title strip, and a margin. */
export const FRAME_HEAD = 26;
export const FRAME_PAD = 14;

/** The height `rows` rows take on a card, the gaps between them included. */
export const rowsHeight = (rows: number): number =>
  rows * CELL + Math.max(0, rows - 1) * ROW_GAP;

/** The width `cols` columns take on a card, the gaps between them included. */
export const colsWidth = (cols: number): number =>
  cols * CELL + Math.max(0, cols - 1) * COL_GAP;

/** The font size a node's header name needs to be READ WHOLE, in px — see docs/graphBuild.md. */
export const HEADER_MAX_PX = 12.5; // .dgx-nm's size today, and the ceiling
export const HEADER_MIN_PX = 7.5;
const GLYPH_W = 0.55; // average advance per px of font size, for the header's face

/** What a module ref is CALLED — the last segment, the whole ref staying as the header's `title`. */
export function moduleDisplayName(ref: string): string {
  const cut = ref.lastIndexOf("/");
  const name = cut >= 0 ? ref.slice(cut + 1) : ref;
  return name || ref;
}

export function headerFontPx(label: string, availW: number): number {
  const n = Math.max(1, label.length);
  const byWidth = availW / (n * GLYPH_W);
  return Math.max(HEADER_MIN_PX, Math.min(HEADER_MAX_PX, byWidth));
}

/** The tab column's width: `TAB_COL_W` when the card draws its tabs as a column, else nothing. */
export const columnW = (ui: PageModuleUI | undefined): number =>
  hasTabs(ui) && ui?.tabsAs !== "dropdown" ? TAB_COL_W : 0;

/** …and the tab MENU's height, when a narrow card offers its tabs across its top instead. */
export const menuH = (ui: PageModuleUI | undefined): number =>
  hasTabs(ui) && ui?.tabsAs === "dropdown" ? TAB_MENU_H : 0;

const sizes = new WeakMap<PageModuleUI, { w: number; h: number }>();

/** How big a node must be to hold `ui`: asked of Rust once per panel object, which the page then keeps. */
export function nodeSize(
  ui?: PageModuleUI,
  footer = false,
): { w: number; h: number } {
  if (!ui || footer) return nodeSizeOp(ui, footer);
  let size = sizes.get(ui);
  if (!size) sizes.set(ui, (size = nodeSizeOp(ui)));
  return size;
}

/** A widget's box in node-local coordinates (origin = the node's top-left), `left` being the tab column's width. */
export function widgetRect(
  w: PageWidget,
  left = 0,
  top = 0,
): {
  x: number;
  y: number;
  w: number;
  h: number;
} {
  return {
    x: left + UI_PAD + w.col * (CELL + COL_GAP),
    y: top + UI_CONTENT_TOP + w.row * (CELL + ROW_GAP),
    w: colsWidth(Math.max(1, w.colSpan ?? 1)),
    h: rowsHeight(Math.max(1, w.rowSpan ?? 1)),
  };
}

/** `widgetRect` on a STRIP drawn taller than its grid: the fader takes the extra height and what sits under it moves down (docs/graphBuild.md). */
export function stripWidgetRect(
  n: { strip?: boolean; ui?: PageModuleUI; h: number },
  w: PageWidget,
  left = 0,
  top = 0,
): { x: number; y: number; w: number; h: number } {
  const r = widgetRect(w, left, top);
  const fader = n.strip
    ? Object.values(n.ui?.widgets ?? {}).find((x) => x.type === "fader")
    : undefined;
  const extra = fader ? n.h - nodeSize(n.ui).h : 0;
  if (!fader || extra <= 0) return r;
  if (w.type === "fader" && w.bind === fader.bind)
    return { ...r, h: r.h + extra };
  const under =
    w.col === fader.col && w.row >= fader.row + (fader.rowSpan ?? 1);
  return under ? { ...r, y: r.y + extra } : r;
}
