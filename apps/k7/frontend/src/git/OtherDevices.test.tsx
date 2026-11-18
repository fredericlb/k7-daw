import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { OtherList } from "./OtherDevices";
import { emptyRoam, offered, type RoamAnswer, roamAfter } from "./roamState";

const answer: RoamAnswer = {
  available: true,
  here: ["h"],
  entries: [
    { id: "a", name: "Band", remote: "git@github.com:o/band.git", addedAt: 1 },
    { id: "b", name: "Notes", remote: null, addedAt: 2 },
    { id: "h", name: "Here", remote: "git@github.com:o/here.git", addedAt: 3 },
  ],
};

const html = (a: RoamAnswer, local: string[] = []) =>
  renderToStaticMarkup(
    <OtherList
      entries={offered(roamAfter(emptyRoam, { type: "loaded", answer: a }))}
      local={local}
      blocked=""
      onCloned={() => {}}
    />,
  );

describe("the section on the home page", () => {
  it("names the workspace and its remote with a Clone button, and skips the unconnected and the present", () => {
    const out = html(answer);
    expect(out).toContain("On your other devices");
    expect(out).toContain('data-roam="a"');
    expect(out).toContain("github.com/o/band");
    expect(out).toContain("Clone");
    expect(out).not.toContain("Notes");
    expect(out).not.toContain("Here");
  });

  it("is absent when nothing is offered", () => {
    expect(html({ ...answer, entries: [answer.entries[1]] })).toBe("");
    expect(html({ available: false, entries: [], here: [] })).toBe("");
  });
});
