// studios.ts — which studio the app views open, and where each of its apps answers. The studio is the
// phone's own backend; each app's address comes from serviceUrl, the one mapping UserMenu and the apps
// themselves use (docs/addresses.md).

import { localServiceUrl, serviceUrl } from "@studio/web-protocol/services";
import { APP_IDS, type AppId } from "./native";

/** k7's origin for each studio the app opens: the phone's own backend, the only one. */
export function studioOrigins(): string[] {
  return [localServiceUrl("k7")];
}

/** Every tab's address on the studio at `origin`. */
export function appUrls(origin: string): Record<AppId, string> {
  return Object.fromEntries(
    APP_IDS.map((id) => [id, serviceUrl(origin, id)]),
  ) as Record<AppId, string>;
}

const CHOSEN = "k7-android.studio";

/** The studio the user last picked, if it is still offered; else the first one. */
export function pickStudio(origins: string[]): string | null {
  let kept: string | null = null;
  try {
    kept = localStorage.getItem(CHOSEN);
  } catch {
    // No storage: the first studio will do.
  }
  if (kept && origins.includes(kept)) return kept;
  return origins[0] ?? null;
}

export function rememberStudio(origin: string) {
  try {
    localStorage.setItem(CHOSEN, origin);
  } catch {
    // Only a convenience.
  }
}
