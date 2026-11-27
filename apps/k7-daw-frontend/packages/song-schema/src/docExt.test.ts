import { describe, expect, it } from "vitest";
import { docPattern } from "./docExt";

describe("docExt", () => {
  it("builds a pattern around the extension", () => {
    const re = docPattern("(^|/)curves/[^/]+");
    expect(re.test("p/curves/a.k7x")).toBe(true);
    expect(re.test("curves/a.K7X")).toBe(true);
    expect(re.test("curves/a.yaml")).toBe(false);
    expect(re.test("p/curves/x/a.k7x")).toBe(false);
    expect(docPattern("^clips/[^/]+", "").test("clips/a.K7X")).toBe(false);
  });
});
