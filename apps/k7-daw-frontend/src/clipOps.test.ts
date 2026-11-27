// The order the chains of the "run a chain over this clip" menu are listed in; the filter and the line are Rust's.

import { describe, expect, it } from "vitest";
import { chainOps } from "./clipOps";

describe("chainOps", () => {
  it("names every chain by its whole `@<library>/…` ref, the one the verb takes", () => {
    expect(
      chainOps([{ ref: "@k7-core/mastering/loud" }, { ref: "@drums/clean" }]),
    ).toEqual(["@drums/clean", "@k7-core/mastering/loud"]);
  });

  // The service answers in code points, capitals first; the menu reads as a person does.
  it("lists them for people, a folder's chains together", () => {
    expect(
      chainOps([
        { ref: "@k7-core/Vocal" },
        { ref: "@k7-core/mastering/loud" },
        { ref: "@k7-core/dry" },
      ]),
    ).toEqual(["@k7-core/dry", "@k7-core/mastering/loud", "@k7-core/Vocal"]);
  });

  it("answers an empty list for a library that has not arrived yet", () => {
    // Both callers can be holding null: one fetches on open, the other rides `refresh()`.
    expect(chainOps(null)).toEqual([]);
    expect(chainOps(undefined)).toEqual([]);
  });
});
