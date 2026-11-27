// recentCommands.ts — the commands this person reached for lately, so the palette can offer
// them first. Kept in localStorage, per browser and NOT per project: it is a habit, and habits
// travel between songs. See docs/commandBar.md.
//
// Storage only. What recency DOES to the ranking is `rankCommands` in `@studio/k7-wasm/ui`, which takes
// the list as an argument and stays pure — the half worth testing is the half without a disk.

const KEY = "k7.recentCommands";

// How many are remembered is Rust's: the ranking's bonus decays across exactly that many places.
import { MAX_RECENT } from "@studio/k7-wasm/ui";

/** Most recent first, and silent about a storage that refuses to answer — Safari throws on READING it in some private modes. */
export function recentCommands(): string[] {
  try {
    const raw = localStorage.getItem(KEY);
    const parsed = raw ? JSON.parse(raw) : null;
    return Array.isArray(parsed)
      ? parsed.filter((x): x is string => typeof x === "string")
      : [];
  } catch {
    return [];
  }
}

/** Push `name` to the front, deduped. `null` is the no-op a line that names no command gets. */
export function rememberCommand(name: string | null | undefined): void {
  if (!name) return;
  try {
    const next = [name, ...recentCommands().filter((c) => c !== name)].slice(
      0,
      MAX_RECENT,
    );
    localStorage.setItem(KEY, JSON.stringify(next));
  } catch {
    /* no storage, no history — the palette still works */
  }
}
