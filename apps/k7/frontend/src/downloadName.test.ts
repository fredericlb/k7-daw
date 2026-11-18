// downloadName.test.ts — a mixdown saves under its own name, read from `filename*` before the ASCII fallback.

import { describe, expect, it } from "vitest";
import { downloadName } from "./api";

const header = (name: string) => {
  const plain = [...name]
    .map((c) => (/^[\x20-\x7e]$/.test(c) ? c : "_"))
    .join("");
  return `attachment; filename="${plain}"; filename*=UTF-8''${encodeURIComponent(name)}`;
};

describe("downloadName", () => {
  it("reads accented and non-latin names from filename*", () => {
    expect(downloadName(header("Été.wav"))).toBe("Été.wav");
    expect(downloadName(header("日本.wav"))).toBe("日本.wav");
    expect(downloadName(header("plain.wav"))).toBe("plain.wav");
  });
  it("falls back to filename without filename*, or with a bad escape", () => {
    expect(downloadName('attachment; filename="a.wav"')).toBe("a.wav");
    expect(
      downloadName("attachment; filename=\"a.wav\"; filename*=UTF-8''%C3"),
    ).toBe("a.wav");
    expect(downloadName("")).toBeNull();
  });
});
