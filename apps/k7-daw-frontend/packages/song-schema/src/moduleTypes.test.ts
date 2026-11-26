import { describe, expect, it } from "vitest";
import { completionOrder, type ModuleStatus } from "./moduleTypes";

// The module rules (`isEngineRef`, `moduleStatusOf`…) are the Rust crate's, tested in k7-wasm's schema_tests.rs.

describe("completionOrder", () => {
  const status = (m: string): ModuleStatus =>
    m.startsWith("fav")
      ? "favorite"
      : m.startsWith("hid")
        ? "hidden"
        : "listed";

  it("floats the favourites and drops the hidden", () => {
    expect(
      completionOrder(["amp", "fav-bass", "hid-test", "verb"], status),
    ).toEqual(["fav-bass", "amp", "verb"]);
  });

  // The contract: an alphabetical list comes back alphabetical, in two blocks (docs/moduleTypes.md).
  it("is STABLE within each group — the caller's order survives", () => {
    expect(
      completionOrder(["fav-a", "b", "fav-c", "d", "fav-e"], status),
    ).toEqual(["fav-a", "fav-c", "fav-e", "b", "d"]);
  });

  it("leaves a list with nothing to say about status exactly as it was", () => {
    const plain = ["a", "b", "c"];
    expect(completionOrder(plain, () => "listed")).toEqual(plain);
  });

  it("can empty a list, and that is a legal answer", () => {
    expect(completionOrder(["hid-1", "hid-2"], status)).toEqual([]);
  });
});
