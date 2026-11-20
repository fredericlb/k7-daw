// The Export panel: the tracks zip row greyed while a render runs, the folder step, and the result's actions.

import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { parseExportAsk, tracksLine } from "../exportArgs";
import { SharePanel } from "./SharePanel";

const render = (busy: boolean) =>
  renderToStaticMarkup(<SharePanel busy={busy} onRun={() => {}} />);

// The button holding the label, tag to close.
const tracksButton = (html: string) =>
  html.match(
    /<button[^>]*>(?:(?!<\/button>).)*Export tracks \(zip\)(?:(?!<\/button>).)*<\/button>/s,
  )?.[0] ?? "";

const desktopHost = { canPick: true, canReveal: true, canShare: false };
const saved = {
  folder: { id: "/m", name: "Music" },
  name: "mix.wav",
  where: "/m/mix.wav",
};

describe("the folder step", () => {
  it("offers the picker where the host has one", () => {
    const html = renderToStaticMarkup(
      <SharePanel busy={false} host={desktopHost} onRun={() => {}} />,
    );
    expect(html).toContain("Choose a folder…");
  });

  it("downloads in a browser, which has no picker", () => {
    expect(render(false)).toContain("Download");
    expect(render(false)).not.toContain("Choose a folder…");
  });

  it("names the folder of the last file, with the actions the host has", () => {
    const html = renderToStaticMarkup(
      <SharePanel
        busy={false}
        host={desktopHost}
        last={saved}
        onRun={() => {}}
      />,
    );
    expect(html).toContain("Saved in Music");
    expect(html).toContain("Show in folder");
    expect(html).not.toContain("Share…");
  });
});

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
