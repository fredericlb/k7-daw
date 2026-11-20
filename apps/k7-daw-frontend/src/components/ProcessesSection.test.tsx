// ProcessesSection.test.tsx — a failed process is listed with its failure text, which the header spinner never shows.
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { setServiceProcesses } from "../processes";
import { ProcessesSection } from "./ProcessesSection";

describe("the processes section", () => {
  it("is empty with nothing listed", () => {
    setServiceProcesses([]);
    expect(renderToStaticMarkup(<ProcessesSection />)).toBe("");
  });

  it("names a failed process and says why", () => {
    setServiceProcesses([
      {
        id: "p1",
        label: "track transcribe drums",
        state: "failed",
        startedAt: 0,
        endedAt: 3000,
        detail: "no model",
      },
    ]);
    const html = renderToStaticMarkup(<ProcessesSection />);
    expect(html).toContain("track transcribe drums");
    expect(html).toContain("Failed");
    expect(html).toContain("no model");
    setServiceProcesses([]);
  });
});
