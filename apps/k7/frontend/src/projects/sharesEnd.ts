// sharesEnd.ts — when sharing projects by link ends, and whether this browser has dismissed the notice saying so.
// The date is shown as typed, in both languages: it is one line to change when the real date is known.

/** The closing date, spelt as the notice says it (placeholder until the date is announced). */
export const SHARES_END = "next week";

const DISMISSED_KEY = "k7.sharesEnd.dismissed";

/** Whether this browser dismissed the notice; a browser refusing storage shows it each time. */
export function sharesEndDismissed(): boolean {
  try {
    return localStorage.getItem(DISMISSED_KEY) === "1";
  } catch {
    return false;
  }
}

/** Remember the dismissal; refused storage costs only the memory. */
export function dismissSharesEnd(): void {
  try {
    localStorage.setItem(DISMISSED_KEY, "1");
  } catch {
    /* the notice comes back next visit */
  }
}
