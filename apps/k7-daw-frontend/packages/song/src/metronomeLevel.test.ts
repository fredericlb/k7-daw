// The metronome's LEVEL, from settings.k7x to the compiled document.
//
// Its range is 0.2 … 1 and that floor is not a taste: Tracktion's `Edit::setClickTrackVolume`
// clamps there, so a smaller number would be written down and then played at 0.2 — a file stating
// a level nobody hears. Silence is `metronome: false`, a different field and a different question.
// See docs/metronome.md.

import {
  DEFAULT_METRONOME_LEVEL,
  METRONOME_MIN,
} from "@studio/engine-protocol/build";
import { SettingsSchema } from "@studio/song-schema/yaml/projectSchema";
import { describe, expect, it } from "vitest";

describe("settings.k7x's metronome_level", () => {
  // A project that never mentions it clicks at Tracktion's own default, so nothing on disk moved.

  /** CLAMPED on the way in, so a hand-edited file lands on the number both sides agree on. */

  // …and the SCHEMA refuses rather than clamps: a value meaning something else is worse than an error.
  it("is refused outright by the schema when out of range", () => {
    expect(SettingsSchema.safeParse({ metronome_level: 0.5 }).success).toBe(
      true,
    );
    expect(SettingsSchema.safeParse({ metronome_level: 0 }).success).toBe(
      false,
    );
    expect(SettingsSchema.safeParse({ metronome_level: 1.5 }).success).toBe(
      false,
    );
  });

  // Independent on purpose: one field could not bring back the level after the click goes off.
});
