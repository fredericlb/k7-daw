// controls.ts — what a `mastering` surface has knobs for: the controls in three groups, one per part of the quick-master node.
// The table is Rust's (`MASTER_CONTROLS`, in the chains crate); only the translated labels are here, keyed by the control's id.
// See ../../docs/mastering.md.

import { unmappedControls as unmapped } from "@studio/k7-wasm/chains";
import { MASTER_PARTS, MASTER_CONTROLS as TABLE } from "@studio/k7-wasm/consts";
import { masterModules } from "@studio/k7-wasm/surfaces";
import type { MixView } from "@studio/k7-wasm/text";
import { m } from "../paraglide/messages.js";

export { MASTER_PARTS };

/** The three appliances inside one quick-master node, in the order the signal meets them. */
export type MasterPart = (typeof MASTER_PARTS)[number];

export const PART_LABEL: Record<MasterPart, () => string> = {
  comp: () => m["master.partComp"](),
  spread: () => m["master.partSpread"](),
  limiter: () => m["master.partLimiter"](),
};

export interface MasterControl {
  /** The mapping key. Prefixed by its part, which is what groups the panel. */
  id: string;
  label: () => string;
  part: MasterPart;
  /** The fallback range, in the PARAM's own units — a fallback and not a scale: the module's own widget wins when it draws one. */
  range: [number, number];
  /** Where the knob sits when nothing is known yet — never 0, which is a real value on most of these. */
  home: number;
  step: number;
  /** Printed after the value — cosmetic. */
  unit: string;
  /** The note the scaffold prints beside the empty line. */
  wants: string;
  /** Not required by the gate: a surface written before it existed still draws (docs/mastering.md). */
  optional?: true;
  /** A CHOICE rather than a range: the param's values 0, 1, 2… named in order, drawn as a dropdown. */
  choices?: readonly string[];
}

const LABELS: Record<string, () => string> = {
  "comp.lowCut": () => m["master.lowCut"](),
  "comp.low": () => m["master.low"](),
  "comp.mid": () => m["master.mid"](),
  "comp.high": () => m["master.high"](),
  "comp.lowFreq": () => m["master.lowCross"](),
  "comp.highFreq": () => m["master.highCross"](),
  "spread.width": () => m["master.width"](),
  "spread.bassMono": () => m["master.bassMono"](),
  "limiter.hfLimit": () => m["master.hfLimit"](),
  "limiter.drive": () => m["master.drive"](),
  "limiter.ceiling": () => m["master.ceiling"](),
  "limiter.release": () => m["master.release"](),
  "limiter.output": () => m["master.output"](),
};

/** The controls in signal order, with NO normalisation anywhere on this surface, so a number can be checked against a meter. */
export const MASTER_CONTROLS: readonly MasterControl[] = (
  TABLE as unknown as readonly Omit<MasterControl, "label">[]
).map((c) => ({ ...c, label: LABELS[c.id] }));

export const masterControl = (id: string): MasterControl | undefined =>
  MASTER_CONTROLS.find((c) => c.id === id);

/** The REQUIRED controls with no module behind them — a missing line being a knob that does nothing (docs/mastering.md). */
export function unmappedControls(
  mapping: MixView | undefined,
): readonly MasterControl[] {
  return unmapped(
    MASTER_CONTROLS.filter((c) => !c.optional),
    mapping,
  );
}

/** The modules this surface's mapping names, deduplicated — more than one is legal, and worth SAYING rather than refusing. */
export const mappedModules = (mapping: MixView | undefined): string[] =>
  masterModules(mapping);
