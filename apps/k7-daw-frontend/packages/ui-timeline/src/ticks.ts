// ticks.ts — a ruler's unit for its zoom, which gridlines get a LABEL, and the tick a click landed on: Rust's, re-exported where the surfaces already import them.
// A reader would get wrong that nothing is computed here: the arithmetic is apps/k7-daw-wasm/gesture's, reached through @studio/k7-wasm (docs/ticks.md).

export {
  adjacentTick,
  chooseTick,
  formatClock,
  formatSecondTick,
  labelled,
  labelMinPx,
  labelStride,
  maxColumnPx,
  nearestTick,
  secondTicks,
  subdivideBeats,
  subdivideGrid,
  ticksUpTo,
} from "@studio/k7-wasm/gestureTicks";
