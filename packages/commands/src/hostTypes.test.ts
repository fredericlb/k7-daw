// isNoFfmpeg: a verb catches a missing tool by NAME, since the server's class and the Worker's share nothing else.

import { NoFfmpegError as ServerError } from "@studio/video-prep/probe";
import { describe, expect, it } from "vitest";
import { isNoFfmpeg, NoFfmpegError } from "./hostTypes.ts";

describe("isNoFfmpeg", () => {
  it("knows this package's error, the server's, and an error rebuilt by name", () => {
    expect(isNoFfmpeg(new NoFfmpegError("ffprobe"))).toBe(true);
    expect(isNoFfmpeg(new ServerError("ffprobe", "needed"))).toBe(true);
    // A host maps its 503 code to a fresh Error named so, which is all a cloned error keeps.
    const rebuilt = new Error("x");
    rebuilt.name = "NoFfmpegError";
    expect(isNoFfmpeg(rebuilt)).toBe(true);
  });

  it("knows an error from another realm by its name, and nothing else", () => {
    expect(isNoFfmpeg({ name: "NoFfmpegError" })).toBe(true);
    expect(isNoFfmpeg(new Error("ffprobe is gone"))).toBe(false);
    expect(isNoFfmpeg({ name: "Error" })).toBe(false);
    expect(isNoFfmpeg(null)).toBe(false);
  });
});
