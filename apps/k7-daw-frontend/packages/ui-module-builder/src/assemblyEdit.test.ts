// A template the writer refuses reaches the person as a sentence; the text is left as it was.

import { describe, expect, it } from "vitest";
import { assemblyProblem, editedAssembly } from "./assemblyEdit";

const BAD = 'label("x")\nplugs([])\n';

describe("an assembly template that does not read", () => {
  it("is left as it was, with the writer's reason", () => {
    const r = editedAssembly(BAD, { op: "dropParam", name: "Trim" });
    expect(r.text).toBe(BAD);
    expect(r.error).toBeTruthy();
  });

  it("names the line of the finding", () => {
    expect(assemblyProblem(BAD)).toMatch(/^line 2:/);
  });
});
