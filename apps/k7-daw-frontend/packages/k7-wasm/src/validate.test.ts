// validate.test.ts — the editor's check reaches the module, and `firstIssue` phrases a verdict for a status line.
// The rules (what each folder's reader judges, where a finding lands) are song's: song/tests/all/files_validate*.rs.

import { describe, expect, it } from "vitest";
import { firstIssue, validateFile, validatePanel } from "./validate.ts";

describe("the editor's check", () => {
  it("answers a verdict the editor draws, a finding with its line, column and path", () => {
    const text = 'extends("@workspace/instruments/synth")\nnonsense(1)\n';
    const v = validateFile("song/nodes/pad.k7x", text);
    expect(v).toMatchObject({ ok: false, depth: "schema", malformed: false });
    expect(v.issues[0]).toMatchObject({ line: 2, col: 1, path: "nonsense" });
    expect(validateFile("song/nodes/pad.k7x", 'extends("@a/b")\n').ok).toBe(
      true,
    );
    expect(validatePanel("cols(4)\nrows(2)\nwidgets({})\n").ok).toBe(true);
  });

  it("phrases the first thing wrong for a status line", () => {
    const v = validateFile("song/sequences/riff.k7x", 'to("2:1")\n');
    expect(firstIssue(v)).toContain("notes: missing key");
    expect(firstIssue(validateFile("x.txt", ""))).toBeNull();
  });
});
