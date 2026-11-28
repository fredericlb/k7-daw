import { EditorState } from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { previewTriggers } from "./previewTriggers";

// The ▶ gutter in the buffer's coordinates; which lines earn one is k7x's (tokens.rs, tokens_cases_tests.rs).

const triggers = (doc: string) => {
  const state = EditorState.create({ doc });
  return previewTriggers(state)
    .map((t) => ({ line: state.doc.lineAt(t.pos).number, req: t.req }))
    .sort((a, b) => a.line - b.line);
};

describe("what earns a play button", () => {
  it("a clip's own source, on its line of the buffer", () => {
    expect(triggers('source("kick.wav")\n')).toEqual([
      { line: 1, req: { kind: "source", source: "kick.wav" } },
    ]);
  });
});
