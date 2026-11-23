// i18n.ts — bind Paraglide's generated runtime to React. The catalogues live in
// messages/<locale>.json and compile into src/paraglide/, which components import
// directly. Deliberately NOT localized: the command reference and file syntax (the
// commands are English) and anything fetched from the backend. LocaleProvider is mounted
// above <AuthGate/> in main.tsx — keep it there; nothing rendering a message may sit above it.

import { createLocaleContext } from "@studio/i18n/react";
import { useMemo } from "react";
import { rememberLocale, useLocaleFromPrefs } from "./localePrefs";
import * as runtime from "./paraglide/runtime.js";

const context = createLocaleContext(runtime);

export const { LocaleProvider } = context;

/** The language and its switch; a switch is also stored in the preferences, which the iOS app's native home reads. */
export function useLocale() {
  const value = context.useLocale();
  return useMemo(
    () => ({
      ...value,
      setLocale: (next: typeof value.locale) => {
        value.setLocale(next);
        rememberLocale(next);
      },
    }),
    [value],
  );
}

/** Mounted once under the provider: the page follows the language the preferences hold. */
export function LocaleFromPrefs(): null {
  useLocaleFromPrefs(context.useLocale());
  return null;
}
