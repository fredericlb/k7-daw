// A glued lane row offers a `+` on each joined seam and one past its tail, into what the tail feeds.

import type { WebGraph } from "@studio/graph-model";
import { describe, expect, it } from "vitest";
import { splitTrailingMixer } from "./anchorRight";
import { moduleRow } from "./graph";
import { rowSeams } from "./rowSeams";

/** The room the tail `+` takes past the last card (the `chains` crate's `SEAM_END_ROOM`). */
const SEAM_END_ROOM = 40;

/** The seams between cards alone, the tail left out. */
const between = <S extends { end?: boolean }>(seams: S[]): S[] =>
  seams.filter((s) => !s.end);

const graph = (use: string[], out = [{ to: "MIX", gain: 1 }]): WebGraph => ({
  chains: {
    lead: { from: "gt_lead", use: use.map((module) => ({ module })), out },
  },
  tracks: ["gt_lead"],
  warnings: [],
});

const row = (g: WebGraph, use: string[], gap?: number) =>
  moduleRow(g, use, { asDrawn: true, links: true, track: "gt_lead", gap });

describe("moduleRow's gap", () => {
  it("glues the cards edge to edge at 0", () => {
    const r = row(graph(["amp", "verb"]), ["amp", "verb"], 0);
    for (let i = 1; i < r.nodes.length; i++)
      expect(r.nodes[i].x).toBe(r.nodes[i - 1].x + r.nodes[i - 1].w);
  });
});

describe("a row glued onto shared borders (gap -1)", () => {
  it("overlaps each card on the one before by a pixel, and still finds the seams and glues their arrows", () => {
    const g = graph(["amp", "verb"]);
    const r = row(g, ["amp", "verb"], -1);
    for (let i = 1; i < r.nodes.length; i++)
      expect(r.nodes[i].x).toBe(r.nodes[i - 1].x + r.nodes[i - 1].w - 1);
    const { graph: out, seams } = rowSeams(r, r, g);
    expect(between(seams).map((s) => s.insert)).toEqual([
      { before: "gt_lead", after: "amp" },
      { before: "amp", after: "verb" },
    ]);
    expect(out.edges.every((e) => e.glued)).toBe(true);
  });
});

describe("rowSeams", () => {
  it("puts a seam where two touching cards are joined, and marks that arrow glued", () => {
    const g = graph(["amp", "verb"]);
    const r = row(g, ["amp", "verb"], 0);
    const { graph: out, seams } = rowSeams(r, r, g);
    expect(between(seams).map((s) => s.insert)).toEqual([
      { before: "gt_lead", after: "amp" },
      { before: "amp", after: "verb" },
    ]);
    expect(seams[1].x).toBe(r.nodes[2].x);
    expect(out.edges.every((e) => e.glued)).toBe(true);
  });

  it("offers nothing on a row with air between its cards", () => {
    const g = graph(["amp", "verb"]);
    const r = row(g, ["amp", "verb"]);
    expect(between(rowSeams(r, r, g).seams)).toEqual([]);
  });

  it("adds the tail past the last card, and room for it", () => {
    const g = graph(["amp", "verb"]);
    const r = row(g, ["amp", "verb"], 0);
    const { graph: out, seams } = rowSeams(r, r, g);
    const tail = seams[seams.length - 1];
    expect(tail).toMatchObject({
      end: true,
      insert: { before: "verb", after: "MIX" },
    });
    const last = r.nodes[r.nodes.length - 1];
    expect(tail.x).toBeGreaterThan(last.x + last.w);
    expect(out.width).toBe(r.width + SEAM_END_ROOM);
  });
});

describe("rowExit", () => {
  it("splices before a mixer the row's end was lifted off to", () => {
    const g: WebGraph = {
      ...graph(["amp", "strip"]),
      trackMixers: { gt_lead: "strip" },
    };
    const full = row(g, ["amp", "strip"], 0);
    const split = splitTrailingMixer(full);
    expect(split).not.toBeNull();
    const tail = rowSeams(full, split?.row ?? full, g).seams.find((s) => s.end);
    expect(tail?.insert).toEqual({ before: "amp", after: "strip" });
  });
});

describe("a stack in the row", () => {
  const stacked = (): WebGraph => ({
    ...graph(["amp", "eq", "verb"]),
    stacks: [["amp", "eq"]],
  });

  it("is one column, its cards one under another at the column's width", () => {
    const r = moduleRow(stacked(), ["amp", "eq", "verb"], {
      asDrawn: true,
      links: true,
      track: "gt_lead",
      gap: 0,
      stack: true,
    });
    const [, amp, eq, verb] = r.nodes;
    expect(eq.x).toBe(amp.x);
    expect(eq.y).toBe(amp.y + amp.h);
    expect(eq.w).toBe(amp.w);
    expect(verb.x).toBe(amp.x + amp.w);
  });

  it("glues the arrow down the column, and puts a branch on a seam whose left card is a module", () => {
    const r = moduleRow(stacked(), ["amp", "eq", "verb"], {
      asDrawn: true,
      links: true,
      track: "gt_lead",
      gap: 0,
      stack: true,
    });
    const { graph: out, seams } = rowSeams(r, r, stacked(), true);
    const down = out.edges.find(
      (e) => e.source.endsWith("amp") && e.target.endsWith("eq"),
    );
    expect(down?.glued).toBe(true);
    // Between the columns: the lane into the stack — no branch, a lane being a source — and the stack's last card into verb.
    expect(between(seams).map((s) => [s.insert, s.branch])).toEqual([
      [{ before: "gt_lead", after: "amp" }, undefined],
      [
        { before: "eq", after: "verb" },
        { before: "eq", after: "eq", stack: true },
      ],
    ]);
    expect(out.height).toBe(r.height);
  });

  it("offers no branch unless asked", () => {
    const r = moduleRow(stacked(), ["amp", "eq", "verb"], {
      asDrawn: true,
      links: true,
      gap: 0,
    });
    expect(rowSeams(r, r, stacked()).seams.some((s) => s.branch)).toBe(false);
  });
});
