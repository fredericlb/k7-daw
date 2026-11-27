// presetOrder.test.ts — the scopes keep the service's order; the names inside one read as a person reads them.

import type { PresetEntry } from "@studio/k7-wasm/types";
import { describe, expect, it } from "vitest";
import { presetsForPeople } from "./presetOrder";

const p = (scope: string, name: string) => ({ scope, name }) as PresetEntry;

describe("presetsForPeople", () => {
  it("keeps the scopes in place and sorts the names within each", () => {
    const received = [
      p("workspace", "Lead"),
      p("workspace", "bass 10"),
      p("workspace", "bass 2"),
      p("library", "Pad"),
      p("library", "arp"),
    ];
    expect(presetsForPeople(received)).toEqual([
      p("workspace", "bass 2"),
      p("workspace", "bass 10"),
      p("workspace", "Lead"),
      p("library", "arp"),
      p("library", "Pad"),
    ]);
    expect(received[0].name).toBe("Lead");
  });
});
