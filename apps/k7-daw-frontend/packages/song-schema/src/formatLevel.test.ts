// The project format level: read off the raw text and refused above this build. See ../docs/formatLevel.md.

import { describe, expect, it } from "vitest";
import {
  checkFormatLevel,
  FORMAT_LEVEL,
  FormatTooNewError,
  formatLevelOf,
  formatLevelOfText,
  formatLineOf,
  isFormatTooNew,
} from "./formatLevel";

const NEXT = FORMAT_LEVEL + 1;

describe("formatLevelOfText", () => {
  it("reads a root `format(N)`", () => {
    expect(formatLevelOfText('project({ name: "a" })\nformat(4)\n')).toBe(4);
  });
  it("answers null when the text declares none, and ignores a nested key", () => {
    expect(formatLevelOfText("project({ tempo: 120 })\n")).toBeNull();
    expect(formatLevelOfText("project({\n  format: 9\n})\n")).toBeNull();
  });
  it("finds the line it is declared on", () => {
    expect(formatLineOf("project({\n  tempo: 1\n})\nformat(2)\n")).toBe(4);
  });
});

describe("formatLevelOf", () => {
  it("is the parsed root's `format`, absent meaning 1", () => {
    expect(formatLevelOf({ format: 2 })).toBe(2);
    expect(formatLevelOf({ project: {} })).toBe(1);
    expect(formatLevelOf(null)).toBe(1);
  });
});

describe("FormatTooNewError", () => {
  it("carries its code, both levels and the update message", () => {
    const e = new FormatTooNewError(NEXT, FORMAT_LEVEL, "project.k7x");
    expect(e.code).toBe("format-too-new");
    expect(e.fileLevel).toBe(NEXT);
    expect(e.buildLevel).toBe(FORMAT_LEVEL);
    expect(e.message).toMatch(
      /^project\.k7x: this project was written by a newer k7/,
    );
    expect(e.message).toMatch(/update k7 to open it/);
  });
  it("is thrown above this build's level only", () => {
    expect(() => checkFormatLevel(FORMAT_LEVEL)).not.toThrow();
    expect(() => checkFormatLevel(null)).not.toThrow();
    expect(() => checkFormatLevel(NEXT)).toThrow(FormatTooNewError);
  });
  it("is recognised by its code after a wire hop", () => {
    expect(isFormatTooNew(new FormatTooNewError(NEXT))).toBe(true);
    expect(isFormatTooNew({ code: "format-too-new", fileLevel: NEXT })).toBe(
      true,
    );
    expect(isFormatTooNew(new Error("format-too-new"))).toBe(false);
  });
});
