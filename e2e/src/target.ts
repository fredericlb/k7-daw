// target.ts — the suite's `test`, bound once to a TARGET: a live instance in Chromium, or a phone (Android, iOS).
// Specs ask `k7.open(path)` for a page instead of `page.goto`, because the phone opens a project its own way (docs/e2e.md).
import { test as base, expect, type Page } from "@playwright/test";
import { android } from "./android.ts";
import { FileService } from "./files.ts";
import { ios } from "./ios.ts";

export { expect };

export type Target = "web" | "android" | "ios";

/** Set by playwright.android.config.ts before any spec loads; the web is the default. */
export const TARGET: Target =
  process.env.E2E_TARGET === "android" || process.env.E2E_TARGET === "ios"
    ? process.env.E2E_TARGET
    : "web";

/** What a spec drives, whichever target it runs on. */
export interface K7 {
  target: Target;
  /** A page showing `path`: the one page on the web, the tab holding it in the app. */
  open(path: string): Promise<Page>;
  /** The page showing an address matching `at` after something on `from` navigated there. */
  landed(from: Page, at: RegExp): Promise<Page>;
  /** The signed-in account's file service, reached over HTTP as the browser reaches it. */
  files(): Promise<FileService>;
}

const web = base.extend<{ k7: K7 }>({
  k7: async ({ page, request }, use) => {
    await use({
      target: "web",
      async open(path) {
        await page.goto(path);
        return page;
      },
      async landed(from, at) {
        await expect(from).toHaveURL(at);
        return from;
      },
      files: () => FileService.connect(request),
    });
  },
});

/** The `test` every spec imports: one of the three, chosen when the suite loads. */
export const test = (
  TARGET === "android" ? android : TARGET === "ios" ? ios : web
) as typeof web;
