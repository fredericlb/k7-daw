// wiring.test.ts — the k7x facade reaches the module: a read, a print, a scoped edit with its change log, and a refusal as an error.
// The rules (every exact text, header, scope and printer case) are the Rust crate's: k7x/tests/document_*.rs and read_print.rs and the frozen vectors.

import { describe, expect, it } from "vitest";
import { K7SyntaxError, parse, parseDocument, stringify } from "./index.ts";

describe("the k7x facade", () => {
  it("reads and prints through the module, a syntax error carrying its place", () => {
    expect(parse("a(1) b([2, 3])")).toEqual({ a: 1, b: [2, 3] });
    expect(stringify({ a: 1 }, { header: ["h"] })).toBe("// h\na(1)\n");
    expect(() => parse("a(1)\nb(2,, 3)")).toThrow(K7SyntaxError);
  });

  it("edits through a scoped view and answers the splices that made the text", () => {
    const text = "p({ a: 1 }) // c\n";
    const d = parseDocument(text);
    d.at(["p"]).setIn(["a"], 2);
    expect(d.toString()).toBe("p({ a: 2 }) // c\n");
    const [c] = d.changes;
    expect(text.slice(0, c.start) + c.insert + text.slice(c.end)).toBe(
      d.toString(),
    );
    expect(() => d.setSourceIn(["p", "a"], "1, 2")).toThrow();
  });
});
