// automationDepth.ts — how far a curve moves the control it drives, as the stretch of the control's
// 0..1 travel it sweeps: what a knob draws as its purple gauge. The engine's mapping is the rule —
// a unipolar curve spans 0..amplitude, a bipolar one rides ±amplitude/2 around the knob's set value
// (apps/k7-daw-engine/k7-runtime/engine/EditBuilder.cpp), and a cut keeps one side of that. See docs/graphBuild.md.

import type { Depth } from "@studio/k7-wasm/types";

/** A binding's depth: the curve's amplitude, and whether it rides around the knob rather than from 0 (the build's, in Rust). */
export type AutomationDepth = Depth;

/** The stretch of the control's 0..1 travel the curve sweeps; `base` is where the knob is SET, read only when bipolar. */
export function depthSpan(d: AutomationDepth, base: number): [number, number] {
  const clamp = (v: number) => Math.max(0, Math.min(1, v));
  // Unipolar output is never below 0: a negative cut leaves it nothing to sweep.
  if (!d.bipolar)
    return d.cut === "negative" ? [0, 0] : [0, clamp(d.amplitude)];
  const lo = d.cut === "positive" ? base : base - d.amplitude / 2;
  const hi = d.cut === "negative" ? base : base + d.amplitude / 2;
  return [clamp(lo), clamp(hi)];
}

/** One depth as a string, so a memoised card is handed a value that compares by content. */
export const encodeDepth = (d: AutomationDepth): string =>
  `${d.amplitude} ${d.bipolar ? "bipolar" : "unipolar"} ${d.cut ?? "none"}`;

/** …and back, undefined for anything that is not one. */
export function decodeDepth(
  s: string | undefined,
): AutomationDepth | undefined {
  const [a, p, c] = (s ?? "").split(" ");
  const amplitude = Number(a);
  if (!a || !Number.isFinite(amplitude)) return undefined;
  const cut: { cut?: Depth["cut"] } =
    c === "positive" || c === "negative" ? { cut: c } : {};
  return { amplitude, bipolar: p === "bipolar", ...cut };
}
