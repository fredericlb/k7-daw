// tree.test.ts — what the one order promises: it reaches every level, it reads names the
// way a person does, it is total, and it leaves the listing it was handed alone.

import { describe, expect, it } from "vitest";
import { byPeople, byPeoplePath, sortForPeople, sortNodes } from "./tree";

// The minimum the comparator reads: a folder row and a file row differ only by `children`.
interface Node {
  name: string;
  children?: Node[];
}

const names = (nodes: { name: string }[]) => nodes.map((n) => n.name);

describe("sortNodes", () => {
  it("sorts a level and everything under it", () => {
    const sorted = sortNodes<Node>([
      { name: "wah", children: [{ name: "b" }, { name: "a" }] },
      { name: "amp", children: [] },
    ]);
    expect(names(sorted)).toEqual(["amp", "wah"]);
    expect(names(sorted[1].children ?? [])).toEqual(["a", "b"]);
  });

  // Folders and files mixed: grouping them orders by a thing you cannot see from the name you want.
  it("mixes folders and files", () => {
    const sorted = sortNodes<Node>([
      { name: "solo.wav" },
      { name: "riffs", children: [] },
      { name: "amp.wav" },
    ]);
    expect(names(sorted)).toEqual(["amp.wav", "riffs", "solo.wav"]);
  });

  it("ignores case and accents, and counts numbers", () => {
    expect(names(sortNodes([{ name: "Ébène" }, { name: "eau" }]))).toEqual([
      "eau",
      "Ébène",
    ]);
    expect(
      names(sortNodes([{ name: "prise 10" }, { name: "prise 2" }])),
    ).toEqual(["prise 2", "prise 10"]);
  });

  // The nodes come from React state, so a sort in place would be a mutation nothing asked for.
  it("leaves what it was given untouched", () => {
    const nodes: Node[] = [
      { name: "b", children: [{ name: "z" }, { name: "y" }] },
      { name: "a" },
    ];
    sortNodes(nodes);
    expect(names(nodes)).toEqual(["b", "a"]);
    expect(names(nodes[0].children ?? [])).toEqual(["z", "y"]);
  });
});

describe("byPeople", () => {
  // The code-point order the services answer in puts every capital first, and `_` between the cases.
  it("undoes the code-point order a listing arrives in", () => {
    const received = ["FM Bass", "WT Bass", "_init", "fingered", "wide"];
    expect([...received].sort(byPeople)).toEqual([
      "_init",
      "fingered",
      "FM Bass",
      "wide",
      "WT Bass",
    ]);
  });

  // Total: two names a person cannot tell apart still have one order, whatever the input's.
  it("orders names the collator calls equal", () => {
    expect(byPeople("Bass", "bass")).not.toBe(0);
    expect(byPeople("Bass", "bass")).toBe(-byPeople("bass", "Bass"));
    expect(byPeople("same", "same")).toBe(0);
  });
});

describe("byPeoplePath", () => {
  it("keeps a folder's contents together, under it", () => {
    const paths = ["live set", "Live/b", "Live/A", "Live", "Bis"];
    expect([...paths].sort(byPeoplePath)).toEqual([
      "Bis",
      "Live",
      "Live/A",
      "Live/b",
      "live set",
    ]);
  });
});

describe("sortForPeople", () => {
  it("sorts by one text of each item, into a new array", () => {
    const items = [{ ref: "b" }, { ref: "A" }];
    expect(sortForPeople(items, (i) => i.ref)).toEqual([
      { ref: "A" },
      { ref: "b" },
    ]);
    expect(items[0].ref).toBe("b");
  });
});

describe("sortNodes with foldersFirst", () => {
  it("keeps folders above files at every level", () => {
    const sorted = sortNodes(
      [
        { name: "amp.wav", dir: false },
        { name: "Zed", dir: true, children: [] },
        {
          name: "riffs",
          dir: true,
          children: [
            { name: "b", dir: false },
            { name: "A", dir: true, children: [] },
          ],
        },
      ],
      { foldersFirst: true },
    );
    expect(names(sorted)).toEqual(["riffs", "Zed", "amp.wav"]);
    expect(names(sorted[0].children ?? [])).toEqual(["A", "b"]);
  });
});
