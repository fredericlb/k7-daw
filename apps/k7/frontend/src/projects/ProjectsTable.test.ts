// ProjectsTable.test.ts — which workspace rows take a file dropped on them: none that this device only reads.

import type { AccountWorkspace } from "@studio/web-protocol";
import { describe, expect, it } from "vitest";
import { takesDrops } from "./ProjectsTable";

const ws = (name: string) =>
  ({ name, projects: [] }) as unknown as AccountWorkspace;

describe("takesDrops", () => {
  it("takes a dropped file on an open workspace", () => {
    expect(takesDrops(ws("band"), false)).toBe(true);
  });

  it("hides the drop on a workspace this device only reads", () => {
    expect(takesDrops(ws("band"), true)).toBe(false);
  });
});
