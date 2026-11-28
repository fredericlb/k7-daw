// The panel facade's wiring for tabs and the clipboard: the rules and their cases are the song crate's (modules/ui/edit_tests.rs).

import {
  addTab,
  clipIds,
  copyWidgets,
  pasteWidgets,
  removeTab,
} from "@studio/k7-wasm/panel";
import { describe, expect, it } from "vitest";
import type { ModuleUI, Widget } from "./moduleUi";

const knob = (col: number): Widget => ({
  type: "knob",
  col,
  row: 0,
  colSpan: 2,
  rowSpan: 2,
  bind: "Mix",
});
const plain = (): ModuleUI => ({ cols: 4, rows: 2, widgets: { a: knob(0) } });

describe("tabs and the clipboard through the facade", () => {
  it("makes two tabs from a plain panel and folds back to one", () => {
    const { ui, at } = addTab(plain());
    expect(ui.tabs?.map((t) => t.name)).toEqual(["Main", "Tab 2"]);
    expect(at).toBe(1);
    expect(removeTab(ui, 1).tabs).toBeUndefined();
  });

  it("cuts a block and pastes it under a fresh id, the taken ids passed as a set", () => {
    const widgets = { a: knob(0), b: knob(2) };
    expect(clipIds(widgets, "a")).toEqual(["a"]);
    const clip = copyWidgets(widgets, "a");
    if (!clip) throw new Error("no clip");
    const { widgets: next, first } = pasteWidgets(
      widgets,
      clip,
      { col: 0, row: 2 },
      { cols: 14, rows: 8 },
      new Set(["a"]),
    );
    expect(first).toBe("a_2");
    expect(next.a_2).toMatchObject({ col: 0, row: 2 });
  });
});
