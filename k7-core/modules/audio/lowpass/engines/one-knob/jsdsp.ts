// Low Pass — the DSP itself, as a description the engine runs. Edit this file and the module changes; the
// .json beside it is the build, regenerated whenever this is newer.
//
// The builder ARRIVES as the argument, so nothing is imported at runtime and this folder needs no node_modules.
//
// One knob, 0..100: a single number saying "how muffled".
// The map is GEOMETRIC — 20 kHz down to 200 Hz — since a linear sweep over that span spends nine tenths of the
// knob in territory nobody can hear moving.

import type { GraphBuilder } from "@studio/dsp";

export default (g: GraphBuilder) => {
  const amount = g.param("Amount", { min: 0, max: 100, default: 0 });
  const t = g.mul(amount, 0.01);
  // An ascending pair walked backwards, so 0 sits at an inaudible 20 kHz: packages/dsp/docs/sweep.md.
  g.biquad("lowpass", g.geolerp(200, 20000, g.sub(1, t)), 1);
};
