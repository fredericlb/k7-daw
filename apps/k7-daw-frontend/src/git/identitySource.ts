// identitySource.ts — the words under a commit-author field that was not typed: where the suggestion came from.
// The route names the source (apps/k7-daw-backend/docs/files/git.md); the system account's wording follows the platform.

import { m } from "../paraglide/messages.js";

/** "from GitHub", "from your git config"...; undefined for a typed field or a source this page does not know. */
export function sourceNote(
  source: string | null | undefined,
  mac = typeof navigator !== "undefined" && /^Mac/.test(navigator.platform),
): string | undefined {
  switch (source) {
    case "github":
      return m["git.identityFromGithub"]();
    case "forgejo":
      return m["git.identityFromForgejo"]();
    case "git-config":
      return m["git.identityFromConfig"]();
    case "system":
      return mac ? m["git.identityFromMac"]() : m["git.identityFromComputer"]();
    default:
      return undefined;
  }
}
