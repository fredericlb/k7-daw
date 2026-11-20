import { describe, expect, it } from "vitest";
import { cueRefs, parseMix } from "./mixFile";

// What is pinned here is the REF: a record's workspace path, as the set list names it.

describe("mix.k7x", () => {
  const parse = (text: string) => parseMix(text);

  it("reads the songs it has cues for", () => {
    const { value, issues } = parse(
      'cues(["records/a/b/c", "records/d/e/f"])\n',
    );
    expect(issues).toEqual([]);
    expect(cueRefs(value)).toEqual(["records/a/b/c", "records/d/e/f"]);
  });

  it("is UNORDERED: the file's line order is not the running order", () => {
    const { value } = parse('cues(["records/z/z/z", "records/a/a/a"])\n');
    // Sorted here rather than in each surface, so two screens cannot draw one set two ways.
    expect(cueRefs(value)).toEqual(["records/a/a/a", "records/z/z/z"]);
  });

  it("drops a duplicate instead of refusing the file", () => {
    // Two people merging their halves of a set should not have to resolve that by hand.
    const { value, issues } = parse(
      'cues(["records/a/b/c", "records/a/b/c"])\n',
    );
    expect(issues).toEqual([]);
    expect(cueRefs(value)).toEqual(["records/a/b/c"]);
  });

  it("is empty, not broken, when nothing has been cued yet", () => {
    expect(cueRefs(parse("cues([])\n").value)).toEqual([]);
    expect(cueRefs(parse("").value)).toEqual([]);
  });

  it("refuses a library ref or a path leaving the workspace, on its line", () => {
    // A ref the set list cannot name is a cue nobody can reach.
    const { value, issues } = parse(
      'cues([\n  "@k7-core/songs/a/b/c",\n  "../x.wav",\n])\n',
    );
    expect(value).toBe(null);
    expect(issues[0].message).toContain("workspace audio file");
    expect(issues[0].line).toBe(2);
  });

  it("refuses an unknown key rather than ignoring it", () => {
    // `songs` in particular, the near miss for `cues` — a hand-carried set gets told.
    const { value, issues } = parse("cues([])\nsongs([])\n");
    expect(value).toBe(null);
    expect(issues.length).toBe(1);
    expect(issues[0].line).toBe(2);
  });

  it("reports a syntax error with its line", () => {
    const { value, issues } = parse("cues([\n");
    expect(value).toBe(null);
    expect(issues[0].line).toBeGreaterThan(0);
  });
});
