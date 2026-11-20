import { describe, expect, it } from "vitest";
import { PANEL_DEFS, shownPanel, TOOLBAR_PANELS } from "./panels";

describe("shownPanel", () => {
  it("names the right panel that holds each legacy ?panel= value", () => {
    expect(shownPanel("event", null)).toBe("events");
    expect(shownPanel("mix", null)).toBe("mixer");
    expect(shownPanel("graph", null)).toBe("chain");
    expect(shownPanel("logs", null)).toBe("logs");
  });

  it("falls back to the store's panel, and to none", () => {
    expect(shownPanel(null, "export")).toBe("export");
    expect(shownPanel(null, null)).toBeNull();
    expect(shownPanel(null, "comments")).toBeNull();
  });
});

describe("the toolbar", () => {
  it("has a button for each of its panels, and the segments share one", () => {
    for (const id of TOOLBAR_PANELS) expect(PANEL_DEFS[id].button).toBe(id);
    expect(PANEL_DEFS.references.button).toBe("resources");
    expect(PANEL_DEFS.history.button).toBe("tasks");
  });
});
