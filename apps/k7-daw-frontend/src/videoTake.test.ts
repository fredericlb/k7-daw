// The container a take records in: the browser's choice among the ones the module knows. Its names are Rust's, and pinned in song-edit's tests.

import { describe, expect, it } from "vitest";
import { pickVideoMime, VIDEO_MIMES } from "./videoTake";

describe("pickVideoMime", () => {
  it("takes the best the browser admits to", () => {
    expect(pickVideoMime((m) => m === "video/webm")).toBe("video/webm");
    expect(pickVideoMime(() => true)).toBe(VIDEO_MIMES[0]);
  });

  it("answers undefined when it admits to none, which asks the recorder to choose", () => {
    expect(pickVideoMime(() => false)).toBeUndefined();
  });
});
