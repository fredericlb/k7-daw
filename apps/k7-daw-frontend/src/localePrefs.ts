// localePrefs.ts — k7's language in the account's preferences, where the iOS app's native home reads it too: a choice
// made on either side is written there, and the page follows what is stored once per page and when the app says so.
// What a reader gets wrong: localStorage still holds it for the first paint; the preferences decide. See ../docs/settings.md.

import { useEffect } from "react";
import { engines } from "./api";

/** The event the iOS app dispatches after its own Settings changed the preferences (apps/k7-daw-ios/k7/HomeHost.swift). */
export const PREFS_CHANGED_EVENT = "k7-prefs-changed";

// The last write, so a read started meanwhile cannot put back the language just left.
let writing: Promise<unknown> = Promise.resolve();
// Read once per page: the provider remounts its subtree on every change, which must not ask again.
let followed = false;

/** Store the chosen language; a failure leaves the choice in this page alone. */
export function rememberLocale(locale: string): void {
  writing = engines.patchPrefs({ locale }).catch(() => {});
}

/** The stored language, when it is one of `locales` and not `current`; null otherwise. */
export function storedLocale(
  stored: unknown,
  current: string,
  locales: readonly string[],
): string | null {
  return typeof stored === "string" &&
    stored !== current &&
    locales.includes(stored)
    ? stored
    : null;
}

interface Runtime<L extends string> {
  locale: L;
  locales: readonly L[];
  setLocale: (locale: L) => void;
}

/** Follow the stored language: once per page, and again when the app says the preferences changed. */
export function useLocaleFromPrefs<L extends string>({
  locale,
  locales,
  setLocale,
}: Runtime<L>): void {
  useEffect(() => {
    let live = true;
    const follow = async () => {
      await writing;
      const p = await engines.prefs().catch(() => null);
      const next = live ? storedLocale(p?.locale, locale, locales) : null;
      if (next) setLocale(next as L);
    };
    if (!followed) {
      followed = true;
      void follow();
    }
    const heard = () => void follow();
    window.addEventListener(PREFS_CHANGED_EVENT, heard);
    return () => {
      live = false;
      window.removeEventListener(PREFS_CHANGED_EVENT, heard);
    };
  }, [locale, locales, setLocale]);
}
