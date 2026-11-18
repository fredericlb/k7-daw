// The share panel's tracks zip row: named in both languages, greyed while a render runs, and its line parses back.

import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { parseExportAsk, tracksLine } from "../exportArgs";
import { SharePanel } from "./SharePanel";

const render = (busy: boolean) =>
  renderToStaticMarkup(
    <SharePanel busy={busy} onRun={() => {}} onClose={() => {}} />,
  );

// The button holding the label, tag to close.
const tracksButton = (html: string) =>
  html.match(
    /<button[^>]*>(?:(?!<\/button>).)*Export tracks \(zip\)<\/button>/s,
  )?.[0] ?? "";

describe("the tracks zip row", () => {
  it("is offered beside the mixdown exports", () => {
    expect(tracksButton(render(false))).not.toBe("");
    expect(tracksButton(render(false))).not.toContain(` disabled=""`);
  });

  it("greys out while a render runs", () => {
    expect(tracksButton(render(true))).toContain(` disabled=""`);
  });

  it("runs a line the command parses, with or without a timeline", () => {
    expect(parseExportAsk("tracks")).toEqual({ kind: "tracks" });
    expect(tracksLine()).toBe("project export tracks");
    expect(
      parseExportAsk(tracksLine("Verse").replace("project export ", "")),
    ).toEqual({
      kind: "tracks",
      timeline: "Verse",
    });
    expect(parseExportAsk("tracks main")).toEqual({ kind: "tracks" });
    expect(parseExportAsk("tracks a b")).toBeNull();
    expect(parseExportAsk("tracks ver/se")).toBeNull();
  });
});
