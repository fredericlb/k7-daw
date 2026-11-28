// fields.test.ts — the editor's completions reach the module: a kind by path, the fields at a key path, the values of a field.
// The vocabulary itself (every field, its order, every closed set) is song's: tests/all/views_fields*.rs.

import { describe, expect, it } from "vitest";
import { fieldsAt, fileKindFor, valuesAt } from "./fields.ts";

describe("the completions", () => {
  it("answers a file's kind, the fields at a key path and a field's closed set", () => {
    expect(fileKindFor("song/chains/gtr.k7x")).toBe("chain");
    expect(fileKindFor("song/notes.txt")).toBeNull();
    expect(fileKindFor(undefined)).toBeNull();
    expect(fieldsAt("modules/audio/cab/module.k7x", ["ui"])).toEqual([
      "cols",
      "rows",
      "widgets",
      "tabs",
      "vertical",
    ]);
    expect(fieldsAt("song/chains/gtr.k7x", ["steps", "to"])).toEqual([
      "chain",
      "level",
      "port",
    ]);
    expect(valuesAt("modules/audio/cab/module.k7x", [], "monitor")).toEqual([
      "off",
      "auto",
      "on",
    ]);
    expect(valuesAt("modules/audio/cab/module.k7x", [], "extends")).toEqual([]);
  });
});
