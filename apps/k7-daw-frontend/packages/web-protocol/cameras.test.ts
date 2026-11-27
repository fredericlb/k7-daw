import { describe, expect, it } from "vitest";
import {
  isRemoteLabel,
  readCameraFrame,
  remoteLabel,
  remoteName,
} from "./cameras.ts";

describe("remote labels", () => {
  it("round-trips a device name", () => {
    const l = remoteLabel("iPad");
    expect(isRemoteLabel(l)).toBe(true);
    expect(remoteName(l)).toBe("iPad");
    expect(isRemoteLabel("Logitech BRIO")).toBe(false);
  });
});

describe("readCameraFrame", () => {
  it("reads a frame and refuses anything else", () => {
    expect(readCameraFrame('{"type":"gone","from":"v1"}')).toEqual({
      type: "gone",
      from: "v1",
    });
    expect(readCameraFrame("nope")).toBeNull();
    expect(readCameraFrame("42")).toBeNull();
  });
});
