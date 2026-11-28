import { EditorState } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { valueTokens } from "./entityValues";

// What entityLinks needs: the Rust scan's values in the buffer's coordinates; the scan's cases are k7x's (tokens_cases_tests.rs).

const at = (doc: string) => valueTokens(EditorState.create({ doc }));
const pairs = (doc: string) => at(doc).map((t) => `${t.field}=${t.value}`);

describe("values and the field they belong to", () => {
  it("reads a flat file", () => {
    expect(pairs('extends("audio/verb")\nir("ir/marshall.wav")\n')).toEqual([
      "extends=audio/verb",
      "ir=ir/marshall.wav",
    ]);
  });

  it("gives offsets that exclude the quotes", () => {
    const doc = "module('synth:lead')\n";
    const t = at(doc)[0];
    expect(doc.slice(t.from, t.to)).toBe("synth:lead");
  });

  it("answers nothing for a buffer that never parsed", () => {
    expect(at('module("reverb"\n')).toEqual([]);
  });
});
