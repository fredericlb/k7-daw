import { describe, expect, it, vi } from "vitest";
import { GitError } from "./gitApi";
import type { GitState } from "./gitTypes";
import { readRound } from "./useMergingWorkspaces";

const t = { base: "/b", headers: () => ({}) };
const merging = { mode: "connected", merge: 2 } as GitState;
const reply = (status: number, body: unknown) =>
  new Response(JSON.stringify(body), { status });

describe("the home page's merge lock", () => {
  it("keeps the last state of a workspace whose read failed", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(reply(500, { error: "x" })),
    );
    const out = await readRound(t, ["a"], new Map([["a", merging]]));
    expect(out.get("a")).toBe(merging);
  });

  it("drops a workspace whose route is missing", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(reply(501, { error: "x" })),
    );
    const out = await readRound(t, ["a"], new Map([["a", merging]]));
    expect(out.has("a")).toBe(false);
    expect(new GitError("", 501, "", {}).status).toBe(501);
  });

  it("reads each workspace's own answer", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockImplementation((u: string) =>
          Promise.resolve(
            reply(
              200,
              u.includes("workspace=a")
                ? { mode: "none" }
                : { mode: "connected", merge: 1 },
            ),
          ),
        ),
    );
    const out = await readRound(t, ["a", "b"], new Map());
    expect(out.get("b")?.merge).toBe(1);
  });
});
