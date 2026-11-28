// The TypeScript half of the ghost rule: how a placement names its material and which one wears no ghost; the counting is Rust's (song reads_ports.rs).

import { describe, expect, it } from "vitest";
import {
  isFirstPlacement,
  materialInstances,
  materialOf,
} from "./clipInstances";
import type { WebClip, WebTrack } from "./types";

const clip = (name: string, start: number) =>
  ({
    id: `${name}#id`,
    type: "midi",
    name,
    start,
    length: 4,
    notes: [],
  }) as WebClip;

const track = (id: string, clips: WebClip[]) =>
  ({
    id,
    name: id,
    generated: false,
    mute: false,
    clips,
  }) as unknown as WebTrack;

describe("clipInstances", () => {
  it("names the material by its ref, falling back to the id", () => {
    expect(materialOf(clip("verse", 0))).toBe("verse");
    expect(materialOf(clip("", 0))).toBe("#id");
  });

  it("marks only the placement the module calls first", () => {
    const inst = materialInstances([
      track("bs", [clip("verse", 8)]),
      track("gtr", [clip("verse", 24)]),
    ]).get("verse");
    expect(inst?.n).toBe(2);
    expect(isFirstPlacement(inst, "bs", 8)).toBe(true);
    expect(isFirstPlacement(inst, "gtr", 24)).toBe(false);
    expect(isFirstPlacement(undefined, "bs", 0)).toBe(false);
  });
});
