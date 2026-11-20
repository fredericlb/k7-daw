import { describe, expect, it } from "vitest";
import {
  DOCK_OF_PANEL,
  PANEL_DEFS,
  PANEL_OF_DOCK,
  shownPanel,
  TOOLBAR_PANELS,
} from "./panels";

describe("shownPanel", () => {
  it("names the right panel the URL-borne one holds, and none for the dock's", () => {
    expect(shownPanel("logs", null)).toBe("logs");
    expect(shownPanel("logs", "export")).toBe("logs");
  });

  it("maps each ?panel= value of the dock to its occupant and back", () => {
    expect(DOCK_OF_PANEL.mix).toBe("mixer");
    expect(DOCK_OF_PANEL.graph).toBe("chain");
    expect(DOCK_OF_PANEL.event).toBe("events");
    for (const [panel, occupant] of Object.entries(DOCK_OF_PANEL))
      expect(PANEL_OF_DOCK[occupant]).toBe(panel);
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
