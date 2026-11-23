import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ConflictCard } from "./ConflictCard";
import { BarBody } from "./GitBar";
import type { Conflict, GitState } from "./gitTypes";

const conn: GitState = {
  mode: "connected",
  branch: "main",
  remote: "https://h/r.git",
  ahead: 0,
  behind: 0,
  changed: 0,
};
const html = (s: GitState | null, failure: string | null = null) =>
  renderToStaticMarkup(
    <BarBody
      state={s}
      failure={failure}
      onAct={() => {}}
      onCommit={() => {}}
      onMerge={() => {}}
    />,
  );

describe("the git bar's rendering", () => {
  it("draws nothing without a repository", () => {
    expect(html({ mode: "none" })).toBe("");
  });

  it("says what to pull and push, and the remote", () => {
    const out = html({ ...conn, ahead: 2, behind: 1 });
    expect(out).toContain("1 to pull");
    expect(out).toContain("2 to push");
    expect(out).toContain("https://h/r.git");
  });

  it("names Save first over a pull with changes, and offers Save for a push alone", () => {
    expect(html({ ...conn, behind: 1, changed: 2 })).toContain("Save first");
    expect(html({ ...conn, ahead: 1 })).toContain(">Save<");
  });

  it("offers the merge and no Save while one waits", () => {
    const out = html({ ...conn, changed: 3, merge: 2 });
    expect(out).toContain("Resolve 2 conflicts");
    expect(out).not.toContain(">Save<");
  });

  it("says a failure as it came", () => {
    expect(html(conn, "ghost.invalid does not store large files")).toContain(
      "does not store large files",
    );
  });
});

const c: Conflict = {
  id: "a",
  path: "song.k7x",
  kind: "k7x",
  keyPath: "tracks.drums.volume",
  base: null,
  mine: "1",
  theirs: "2",
  both: false,
  choice: null,
};
const card = (over: Partial<Conflict>) =>
  renderToStaticMarkup(
    <ConflictCard
      conflict={{ ...c, ...over }}
      chosen={undefined}
      disabled={false}
      onPick={() => {}}
    />,
  );

describe("a conflict's card", () => {
  it("names the key path and offers mine and theirs", () => {
    const out = card({});
    expect(out).toContain("tracks.drums.volume");
    expect(out).toContain("Keep mine");
    expect(out).toContain("Keep theirs");
    expect(out).not.toContain("Keep both");
  });

  it("offers both where the route says so", () => {
    expect(card({ both: true })).toContain("Keep both");
  });

  it("offers keep or remove for a removed file", () => {
    const out = card({ kind: "presence", mine: null });
    expect(out).toContain("Keep the file");
    expect(out).not.toContain("Keep mine");
  });
});
