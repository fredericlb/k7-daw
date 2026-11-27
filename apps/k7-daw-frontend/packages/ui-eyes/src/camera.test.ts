// The constraint a lens match becomes: which lens a label names is Rust's rule, tested there (`eyes/tests/rules.rs`).
// What matters here is NOT refusing: an id that rotated falls back to a picture, and a lane never asks for sound.

import { describe, expect, it } from "vitest";
import { cameraConstraint, matchCamera } from "./camera";

describe("the constraint a match becomes", () => {
  it("names the device when one was found", () => {
    expect(cameraConstraint({ deviceId: "b2" })).toEqual({
      video: { deviceId: "b2" },
      audio: false,
    });
  });

  it("asks for ANY camera otherwise — never `exact`, which would fail instead of falling back", () => {
    expect(cameraConstraint({ deviceId: null })).toEqual({
      video: true,
      audio: false,
    });
  });

  it("never asks for sound: a picture lane has none, and asking would prompt for a microphone", () => {
    expect(cameraConstraint({ deviceId: "a1" }).audio).toBe(false);
  });
});

describe("the module's match, through the page's wrapper", () => {
  it("answers a label's device, and a missing lens with its reason", () => {
    const devices = [
      { deviceId: "a1", kind: "videoinput", label: "FaceTime HD Camera" },
    ];
    expect(matchCamera("facetime", devices).deviceId).toBe("a1");
    expect(matchCamera("Blackmagic", devices).problem).toContain(
      "no camera called",
    );
  });
});
