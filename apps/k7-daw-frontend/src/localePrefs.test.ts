import { describe, expect, it } from "vitest";
import { storedLocale } from "./localePrefs";

describe("storedLocale", () => {
  const locales = ["en", "fr"] as const;

  it("follows a stored language the page speaks and is not speaking", () => {
    expect(storedLocale("fr", "en", locales)).toBe("fr");
  });

  it("leaves the page alone for the same, an unknown or no language", () => {
    expect(storedLocale("en", "en", locales)).toBeNull();
    expect(storedLocale("de", "en", locales)).toBeNull();
    expect(storedLocale(undefined, "en", locales)).toBeNull();
    expect(storedLocale(3, "en", locales)).toBeNull();
  });
});
