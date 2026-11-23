// waveformsWarmed.test.tsx — each bump of the prewarm's count is what a clip's hook reads next, so a blank clip asks again.
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { useWaveformsWarmed, waveformsWarmed } from "./waveformsWarmed";

function Reader() {
  return <span>{useWaveformsWarmed()}</span>;
}

describe("the waveform prewarm's count", () => {
  it("is read anew after each bump", () => {
    const before = Number(renderToStaticMarkup(<Reader />).replace(/\D/g, ""));
    waveformsWarmed();
    waveformsWarmed();
    expect(renderToStaticMarkup(<Reader />)).toBe(`<span>${before + 2}</span>`);
  });
});
