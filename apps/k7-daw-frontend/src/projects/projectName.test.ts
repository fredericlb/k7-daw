// The TypeScript half of projectName: the suffix put on, the refusals spelled in messages, the dropped stem; the rules are Rust's (song reads_ports_misc.rs).

import { describe, expect, it } from "vitest";
import {
  droppedStem,
  projectFolderName,
  workspaceFolderName,
} from "./projectName";

describe("projectFolderName", () => {
  it("puts the chosen application's suffix on, which alone decides where the project opens", () => {
    expect(projectFolderName("hiver", "k7")).toEqual({
      folder: "hiver.k7",
      error: "",
    });
    expect(projectFolderName("hiver.K7", "k7").folder).toBe("hiver.k7");
  });

  it("spells each refusal as a message", () => {
    expect(projectFolderName("chansons/hiver", "k7").error).toMatch(/\//);
    expect(projectFolderName(".caché", "k7").error).toMatch(/dot/);
    expect(projectFolderName("   ", "k7").error).toBeTruthy();
  });
});

describe("workspaceFolderName", () => {
  it("puts no suffix on, and refuses what a project refuses", () => {
    expect(workspaceFolderName("hiver.k7").folder).toBe("hiver");
    expect(workspaceFolderName("a/b").error).toBeTruthy();
  });
});

describe("droppedStem", () => {
  it("drops the extension and keeps the rest, dots included", () => {
    expect(droppedStem("Winter Sketch.dawproject")).toBe("Winter Sketch");
    expect(droppedStem("v1.2 final.dawproject")).toBe("v1.2 final");
    expect(droppedStem("winter")).toBe("winter");
    expect(droppedStem(".dawproject")).toBe("");
  });
});
