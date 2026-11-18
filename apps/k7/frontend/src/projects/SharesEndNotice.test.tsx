// The end-of-shares notice: shown until dismissed, remembered per browser, one date in both languages.

import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import enMessages from "../../messages/en.json";
import frMessages from "../../messages/fr.json";
import { SharesEndNotice } from "./SharesEndNotice";
import { dismissSharesEnd, SHARES_END, sharesEndDismissed } from "./sharesEnd";

/** A Storage held in a Map: the suite runs in node, which has none. */
function memoryStorage(): Storage {
  const items = new Map<string, string>();
  return {
    get length() {
      return items.size;
    },
    clear: () => items.clear(),
    getItem: (k) => items.get(k) ?? null,
    key: (i) => [...items.keys()][i] ?? null,
    removeItem: (k) => void items.delete(k),
    setItem: (k, v) => void items.set(k, v),
  };
}

describe("the end-of-shares notice", () => {
  beforeEach(() => vi.stubGlobal("localStorage", memoryStorage()));
  afterEach(() => vi.unstubAllGlobals());

  it("is shown until it is dismissed", () => {
    expect(renderToStaticMarkup(<SharesEndNotice />)).toContain(SHARES_END);
    dismissSharesEnd();
    expect(sharesEndDismissed()).toBe(true);
    expect(renderToStaticMarkup(<SharesEndNotice />)).toBe("");
  });

  it("is shown again when the browser refuses storage", () => {
    const refusing = () => {
      throw new Error("denied");
    };
    vi.stubGlobal("localStorage", {
      getItem: refusing,
      setItem: refusing,
    });
    expect(sharesEndDismissed()).toBe(false);
    expect(() => dismissSharesEnd()).not.toThrow();
  });

  it("names the date in both languages, from the one constant", () => {
    for (const messages of [enMessages, frMessages])
      expect(messages["home.sharesEnd"]).toContain("{date}");
  });
});
