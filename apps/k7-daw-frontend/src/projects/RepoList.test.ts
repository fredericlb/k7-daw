// RepoList.test.ts — the order the home page lists workspaces and projects in: for people, Examples last.

import type { AccountWorkspace } from "@studio/web-protocol";
import { describe, expect, it } from "vitest";
import { pageOrder } from "./RepoList";

const ws = (name: string, projects: string[] = []) =>
  ({
    name,
    projects: projects.map((path) => ({ path, app: "k7" })),
  }) as unknown as AccountWorkspace;

describe("pageOrder", () => {
  // The service answers in code points, capitals first; the page reads as a person does.
  it("sorts the workspaces for people and keeps Examples last", () => {
    const listed = pageOrder([ws("Zed"), ws("Examples"), ws("band")]);
    expect(listed.map((w) => w.name)).toEqual(["band", "Zed", "Examples"]);
  });

  it("sorts each workspace's projects by path, a folder's projects under it", () => {
    const [w] = pageOrder([
      ws("band", ["Live/b", "live set", "Live/A", "demo 10", "demo 2"]),
    ]);
    expect(w.projects.map((p) => p.path)).toEqual([
      "demo 2",
      "demo 10",
      "Live/A",
      "Live/b",
      "live set",
    ]);
  });
});
