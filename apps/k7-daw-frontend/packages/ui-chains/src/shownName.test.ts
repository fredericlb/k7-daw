// The wiring of the shown names: Rust's map, and the port a looked-up name keeps. The names are tested in the chains crate
// (apps/k7-daw-wasm/chains/src/shown_name.rs).

import { describe, expect, it } from "vitest";
import { shownNames, shownNamesOf } from "./shownName";

const UUID = "30c0f1fa-41cb-499c-b101-705257778fe4";

describe("shownNames", () => {
  const shown = shownNames({
    "ge.trem": {
      use: [
        { module: "ge.trem" },
        { module: UUID, uses: "@k7-core/audio/amp", place: "ge.trem · step 2" },
      ],
    },
  });

  it("shows a node by its place, keeps a port on it and leaves other names alone", () => {
    expect(shown(UUID)).toBe("amp · ge.trem · step 2");
    expect(shown(`${UUID}#side`)).toBe("amp · ge.trem · step 2 #side");
    expect(shown("MIX")).toBe("MIX");
    expect(shownNames(undefined)("ge.trem")).toBe("ge.trem");
  });

  it("reads laid-out nodes, skipping those with no module", () => {
    const of = shownNamesOf([
      { place: "x · step 1" },
      { module: UUID, place: "ge.trem · step 4" },
    ]);
    expect(of(UUID)).toBe("ge.trem · step 4");
  });
});
