// The bar's record button on a rig: offered only once the picture's read has answered (apps/k7-daw-wasm/song/docs/outputRecording.md).

import { describe, expect, it } from "vitest";
import { outputAvailable } from "./useOutputRecording";

describe("outputAvailable", () => {
  it("waits for the picture's read, so an early press does not file the sound alone", () => {
    // The engine runs the recorder before the Worker has compiled the picture: a null picture then is no answer yet.
    expect(
      outputAvailable({
        hasAudio: true,
        hasPicture: false,
        pictureKnown: false,
      }),
    ).toBe(false);
  });

  it("offers a rig with sound and no picture once the read said so", () => {
    expect(
      outputAvailable({
        hasAudio: true,
        hasPicture: false,
        pictureKnown: true,
      }),
    ).toBe(true);
  });

  it("offers a picture with nothing reaching OUT", () => {
    expect(
      outputAvailable({
        hasAudio: false,
        hasPicture: true,
        pictureKnown: true,
      }),
    ).toBe(true);
  });

  it("offers nothing to a rig with neither", () => {
    expect(
      outputAvailable({
        hasAudio: false,
        hasPicture: false,
        pictureKnown: true,
      }),
    ).toBe(false);
  });
});
